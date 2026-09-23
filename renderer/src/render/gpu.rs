//! Zeichnen auf der Grafikkarte.
//!
//! Der Renderlauf stellt je Kachel eine Zeichenliste auf
//! ([`draw_list`](super::metatile::draw_list)): Sprite, Position, fertig
//! sortiert. Hier setzt ein Compute-Shader (`gpu.wgsl`) dieselbe Liste
//! zusammen — je Pixel ein Thread, der seine Sprites in Zeichenreihenfolge
//! durchgeht und ganzzahlig mischt wie [`over`](super::rasterizer::over).
//! Ganzzahlig, weil Gleitkomma auf jeder Karte anders rundet; so liefert
//! jede Karte dasselbe Byte wie die CPU, und ein Test kann das nachprüfen.
//!
//! Die Sprites liegen in einem Atlas auf der Karte und kommen beim ersten
//! Gebrauch hinauf. Ist er voll, wird er geleert: die Kacheln einer Gegend
//! brauchen ein paar hundert Sprites, die Tabelle hat zehntausende.
//!
//! Was die Karte nicht macht: Chunks lesen, Kandidaten suchen, Sprites
//! wählen, WebP schreiben. Das bleibt auf der CPU — die Karte ersetzt nur
//! den Blit, also rund die Hälfte der Zeit je Kachel.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::mpsc::{TryRecvError, channel};

use anyhow::{Context, Result, anyhow, bail};
use image::RgbaImage;

use super::metatile::DrawList;
use super::{Cell, Sprite, SpriteId};

/// Kantenlänge der Zellen, in die eine Kachel zerlegt wird: eine
/// Arbeitsgruppe je Zelle, ein Thread je Pixel. Muss zur
/// `workgroup_size` im Shader passen.
const CELL: u32 = 16;

/// Obergrenze für den Sprite-Atlas. Mehr braucht keine Gegend, und auf
/// einer Onboard-Grafik ist der Speicher der des Systems.
const ATLAS_MAX: u64 = 256 << 20;

/// Was ein Zeichner an Puffern höchstens bindet — Zeichenlisten und
/// Kacheln eines Durchgangs. Weit über dem, was vorkommt.
const WORKER_MAX: u64 = 64 << 20;

/// Ein Sprite-Teil im Atlas: Tabelle, Sprite, Würfel — der Schlüssel aus
/// [`Draw`].
type Key = (u64, SpriteId, Cell);

/// Eine geöffnete Grafikkarte mit dem Shader und dem Sprite-Atlas.
/// Teilen sich alle Threads; jeder holt sich einen [`Worker`].
pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
    atlas: Mutex<Atlas>,
    /// Name und Backend, für die Ausgabe beim Start.
    pub name: String,
}

struct Atlas {
    buffer: wgpu::Buffer,
    capacity: u64,
    used: u64,
    /// Wortindex je Sprite-Teil.
    offsets: HashMap<Key, u32>,
    /// Wie oft der Atlas voll war und geleert wurde.
    leerungen: usize,
}

impl Gpu {
    /// Öffnet die beste Grafikkarte; `None`, wenn keine da ist. Mit
    /// `software` gilt auch ein Software-Adapter (WARP, lavapipe) — für
    /// Tests auf Rechnern ohne Karte; zum Rendern ist er langsamer als
    /// der CPU-Pfad.
    pub fn new(software: bool) -> Result<Option<Gpu>> {
        Gpu::with_atlas(software, ATLAS_MAX)
    }

    /// Wie [`Gpu::new`], mit einer Obergrenze für den Atlas in Bytes.
    pub fn with_atlas(software: bool, atlas_max: u64) -> Result<Option<Gpu>> {
        let Some((adapter, info)) = adapter(software) else {
            return Ok(None);
        };
        let name = format!("{} ({:?})", info.name, info.backend);

        let limits = adapter.limits();
        let binding = atlas_max
            .max(WORKER_MAX)
            .min(limits.max_storage_buffer_binding_size)
            .min(limits.max_buffer_size);
        let capacity = atlas_max.min(binding);
        let required_limits = wgpu::Limits {
            max_storage_buffer_binding_size: binding,
            max_buffer_size: binding,
            ..wgpu::Limits::downlevel_defaults()
        };
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("terranova"),
            required_features: wgpu::Features::empty(),
            required_limits,
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        }))
        .with_context(|| format!("{name} öffnen"))?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("kachel"),
            source: wgpu::ShaderSource::Wgsl(include_str!("gpu.wgsl").into()),
        });
        let buffer = |binding, ty| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let lesen = wgpu::BufferBindingType::Storage { read_only: true };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("kachel"),
            // Höchstens vier Storage-Puffer: so viele erlaubt auch der
            // kleinste Adapter (`downlevel_defaults`).
            entries: &[
                buffer(0, lesen),
                buffer(1, lesen),
                buffer(2, lesen),
                buffer(3, wgpu::BufferBindingType::Storage { read_only: false }),
                buffer(4, wgpu::BufferBindingType::Uniform),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("kachel"),
            bind_group_layouts: &[Some(&layout)],
            ..Default::default()
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("kachel"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let atlas = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("atlas"),
            size: capacity,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Ok(Some(Gpu {
            device,
            queue,
            pipeline,
            layout,
            atlas: Mutex::new(Atlas {
                buffer: atlas,
                capacity,
                used: 0,
                offsets: HashMap::new(),
                leerungen: 0,
            }),
            name,
        }))
    }

    /// Wie oft der Atlas bisher voll war. Für den Test, der das erzwingt.
    pub fn atlas_leerungen(&self) -> usize {
        self.atlas
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .leerungen
    }

    /// Ein Zeichner mit eigenen Puffern für bis zu `tiles` quadratische
    /// Kacheln mit `size` Pixeln Kante je Durchgang. Je Thread einer.
    pub fn worker(&self, tiles: u32, size: u32) -> Worker<'_> {
        let cells_x = size.div_ceil(CELL);
        let cells_per_tile = cells_x * cells_x;
        let make = |label, size, usage| {
            self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            })
        };
        let storage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST;
        let tile_bytes = u64::from(size) * u64::from(size) * 4;
        let params = make(
            "params",
            16,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );
        self.queue
            .write_buffer(&params, 0, &bytes(&[size, size, cells_x, cells_per_tile]));
        // Der Puffer des Atlas wechselt nie; sein Griff reicht für die
        // Bindung, ohne die Sperre.
        let atlas = self
            .atlas
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .buffer
            .clone();
        let mut worker = Worker {
            gpu: self,
            tiles,
            size,
            cells_x,
            atlas,
            instances: make("instances", 64 << 10, storage),
            lists: make("lists", 256 << 10, storage),
            params,
            out: make(
                "out",
                tile_bytes * u64::from(tiles),
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            ),
            readback: make(
                "readback",
                tile_bytes * u64::from(tiles),
                wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            ),
            bind: None,
            inst_bytes: Vec::new(),
            list_data: Vec::new(),
        };
        worker.bind = Some(worker.bind_group());
        worker
    }
}

/// Sucht den Adapter: Vulkan zuerst, derselbe Treiberweg auf Windows und
/// Linux, und mit Mesa auf dem Server ohnehin der einzige. DX12 und GL nur,
/// wenn kein brauchbarer Vulkan-Adapter da ist: WARP in der Windows-CI,
/// eine alte Onboard-Grafik ohne Vulkan-Treiber. Unter den Adaptern die
/// stärkste Karte, auf einem Laptop also nicht die Onboard. `WGPU_BACKEND`
/// und `WGPU_ADAPTER_NAME` übersteuern das wie bei wgpu üblich, etwa
/// `WGPU_ADAPTER_NAME="Basic Render"` für WARP.
fn adapter(software: bool) -> Option<(wgpu::Adapter, wgpu::AdapterInfo)> {
    let vorgabe = std::env::var_os("WGPU_BACKEND").is_some()
        || std::env::var_os("WGPU_ADAPTER_NAME").is_some();
    let mut versuche = Vec::new();
    if !vorgabe {
        let mut vulkan = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
        vulkan.backends = wgpu::Backends::VULKAN;
        versuche.push(vulkan);
    }
    versuche.push(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    for desc in versuche {
        let instance = wgpu::Instance::new(desc);
        let adapter = pollster::block_on(async {
            match wgpu::util::initialize_adapter_from_env(&instance, None).await {
                Ok(adapter) => Ok(adapter),
                Err(_) => {
                    instance
                        .request_adapter(&wgpu::RequestAdapterOptions {
                            power_preference: wgpu::PowerPreference::HighPerformance,
                            ..Default::default()
                        })
                        .await
                }
            }
        });
        let Ok(adapter) = adapter else {
            continue;
        };
        let info = adapter.get_info();
        let hardware = matches!(
            info.device_type,
            wgpu::DeviceType::DiscreteGpu
                | wgpu::DeviceType::IntegratedGpu
                | wgpu::DeviceType::VirtualGpu
        );
        if hardware || software {
            return Some((adapter, info));
        }
    }
    None
}

impl Atlas {
    /// Liefert je Schlüssel den Wortindex im Atlas und lädt hoch, was noch
    /// fehlt. Reicht der Platz nicht, fliegt alles raus und die Sprites
    /// dieses Durchgangs kommen neu.
    fn ensure(&mut self, queue: &wgpu::Queue, keys: &[(Key, &Sprite)]) -> Result<Vec<u32>> {
        let need = |offsets: &HashMap<Key, u32>| -> u64 {
            keys.iter()
                .filter(|(key, _)| !offsets.contains_key(key))
                .map(|(_, sprite)| bytes_of(sprite))
                .sum()
        };
        if self.used + need(&self.offsets) > self.capacity {
            self.offsets.clear();
            self.used = 0;
            self.leerungen += 1;
            let need = need(&self.offsets);
            if need > self.capacity {
                bail!(
                    "die Sprites eines Durchgangs brauchen {:.1} MB, der GPU-Atlas fasst {:.1} MB",
                    need as f64 / 1_048_576.0,
                    self.capacity as f64 / 1_048_576.0
                );
            }
        }
        let mut out = Vec::with_capacity(keys.len());
        for (key, sprite) in keys {
            let offset = match self.offsets.get(key) {
                Some(&offset) => offset,
                None => {
                    queue.write_buffer(&self.buffer, self.used, sprite.image.as_raw());
                    let offset = (self.used / 4) as u32;
                    self.offsets.insert(*key, offset);
                    self.used += bytes_of(sprite);
                    offset
                }
            };
            out.push(offset);
        }
        Ok(out)
    }
}

fn bytes_of(sprite: &Sprite) -> u64 {
    sprite.image.as_raw().len() as u64
}

fn bytes(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|w| w.to_le_bytes()).collect()
}

/// Puffer eines Threads: Zeichenlisten hinauf, fertige Kacheln herunter.
pub struct Worker<'g> {
    gpu: &'g Gpu,
    tiles: u32,
    size: u32,
    cells_x: u32,
    atlas: wgpu::Buffer,
    instances: wgpu::Buffer,
    lists: wgpu::Buffer,
    params: wgpu::Buffer,
    out: wgpu::Buffer,
    readback: wgpu::Buffer,
    bind: Option<wgpu::BindGroup>,
    /// Instanzen, 16 Bytes je Stück, fertig für den Puffer.
    inst_bytes: Vec<u8>,
    list_data: Vec<u32>,
}

impl Worker<'_> {
    fn bind_group(&self) -> wgpu::BindGroup {
        fn entry(binding: u32, buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
            wgpu::BindGroupEntry {
                binding,
                resource: buffer.as_entire_binding(),
            }
        }
        self.gpu
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("kachel"),
                layout: &self.gpu.layout,
                entries: &[
                    entry(0, &self.atlas),
                    entry(1, &self.instances),
                    entry(2, &self.lists),
                    entry(3, &self.out),
                    entry(4, &self.params),
                ],
            })
    }

    /// Vergrössert die Listenpuffer, wenn ein Durchgang mehr braucht.
    fn ensure(&mut self, inst_bytes: u64, list_bytes: u64) {
        let mut neu = false;
        for (buffer, need, label) in [
            (&mut self.instances, inst_bytes, "instances"),
            (&mut self.lists, list_bytes, "lists"),
        ] {
            if buffer.size() < need {
                *buffer = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(label),
                    size: need.next_power_of_two(),
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                neu = true;
            }
        }
        if neu {
            self.bind = Some(self.bind_group());
        }
    }

    /// Zeichnet je Liste eine Kachel. Höchstens so viele, wie der Zeichner
    /// angelegt wurde.
    pub fn render(&mut self, lists: &[DrawList]) -> Result<Vec<RgbaImage>> {
        if lists.is_empty() {
            return Ok(Vec::new());
        }
        assert!(
            lists.len() <= self.tiles as usize,
            "mehr Kacheln als der Zeichner fasst"
        );
        let size = self.size as i32;
        let cells_x = self.cells_x as usize;
        let cells_per_tile = cells_x * cells_x;

        // Erst alles, was den Atlas nicht braucht — und damit keine Sperre:
        // Instanzen, Zellentabelle, Listen. Das Sprite steht in der Instanz
        // vorerst als laufende Nummer; die Atlasadresse kommt zum Schluss.
        self.inst_bytes.clear();
        let table = 2 * lists.len() * cells_per_tile;
        self.list_data.clear();
        self.list_data.resize(table, 0);
        let mut keys: Vec<(Key, &Sprite)> = Vec::new();
        let mut nummer: HashMap<Key, u32> = HashMap::new();
        let mut counts = vec![0u32; cells_per_tile];
        let mut spans: Vec<[usize; 4]> = Vec::new();
        let mut instances = 0usize;
        for (t, list) in lists.iter().enumerate() {
            spans.clear();
            counts.fill(0);
            let first = instances;
            for d in &list.draws {
                let (w, h) = (
                    d.sprite.image.width() as i32,
                    d.sprite.image.height() as i32,
                );
                let (x0, y0) = (d.origin.0.max(0), d.origin.1.max(0));
                let (x1, y1) = ((d.origin.0 + w).min(size), (d.origin.1 + h).min(size));
                if x0 >= x1 || y0 >= y1 {
                    continue;
                }
                let n = *nummer.entry(d.key).or_insert_with(|| {
                    keys.push((d.key, d.sprite));
                    keys.len() as u32 - 1
                });
                for word in [
                    n,
                    w as u32 | (h as u32) << 16,
                    d.origin.0 as u32,
                    d.origin.1 as u32,
                ] {
                    self.inst_bytes.extend_from_slice(&word.to_le_bytes());
                }
                instances += 1;
                let span = [
                    x0 as usize / CELL as usize,
                    y0 as usize / CELL as usize,
                    (x1 - 1) as usize / CELL as usize,
                    (y1 - 1) as usize / CELL as usize,
                ];
                for cy in span[1]..=span[3] {
                    for cx in span[0]..=span[2] {
                        counts[cy * cells_x + cx] += 1;
                    }
                }
                spans.push(span);
            }
            // Zählsortierung: je Zelle ein zusammenhängender Bereich der
            // Indizes, in Zeichenreihenfolge.
            let mut start = self.list_data.len();
            let mut cursor = Vec::with_capacity(cells_per_tile);
            for (cell, &count) in counts.iter().enumerate() {
                let slot = 2 * (t * cells_per_tile + cell);
                self.list_data[slot] = start as u32;
                self.list_data[slot + 1] = count;
                cursor.push(start);
                start += count as usize;
            }
            self.list_data.resize(start, 0);
            for (i, span) in spans.iter().enumerate() {
                for cy in span[1]..=span[3] {
                    for cx in span[0]..=span[2] {
                        let cell = cy * cells_x + cx;
                        self.list_data[cursor[cell]] = (first + i) as u32;
                        cursor[cell] += 1;
                    }
                }
            }
        }
        // Ein leerer Puffer lässt sich nicht binden; ein paar Nullbytes schon.
        if self.inst_bytes.is_empty() {
            self.inst_bytes.resize(16, 0);
        }
        let list_bytes = bytes(&self.list_data);
        self.ensure(self.inst_bytes.len() as u64, list_bytes.len() as u64);
        let gpu = self.gpu;
        gpu.queue.write_buffer(&self.lists, 0, &list_bytes);

        // Jetzt der Atlas. Die Sperre hält, bis der Durchgang abgeschickt
        // ist: die Adressen gelten nur, solange ihn niemand leert.
        // ponytail: eine Sperre für Atlas und Absenden; feiner, wenn die
        // GPU-Seite je bremst.
        let mut atlas = gpu.atlas.lock().unwrap_or_else(|e| e.into_inner());
        let offsets = atlas.ensure(&gpu.queue, &keys)?;
        for inst in self
            .inst_bytes
            .as_chunks_mut::<16>()
            .0
            .iter_mut()
            .take(instances)
        {
            let n = u32::from_le_bytes([inst[0], inst[1], inst[2], inst[3]]) as usize;
            inst[..4].copy_from_slice(&offsets[n].to_le_bytes());
        }
        gpu.queue.write_buffer(&self.instances, 0, &self.inst_bytes);

        let tile_bytes = u64::from(self.size) * u64::from(self.size) * 4;
        let out_bytes = tile_bytes * lists.len() as u64;
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("kachel"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("kachel"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&gpu.pipeline);
            pass.set_bind_group(0, self.bind.as_ref().expect("angelegt"), &[]);
            pass.dispatch_workgroups(self.cells_x, self.cells_x, lists.len() as u32);
        }
        encoder.copy_buffer_to_buffer(&self.out, 0, &self.readback, 0, Some(out_bytes));
        let index = gpu.queue.submit([encoder.finish()]);
        drop(atlas);

        let (tx, rx) = channel();
        let slice = self.readback.slice(..out_bytes);
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        gpu.device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(index),
                timeout: None,
            })
            .map_err(|e| anyhow!("auf die GPU warten: {e:?}"))?;
        loop {
            match rx.try_recv() {
                Ok(result) => break result.map_err(|e| anyhow!("Kacheln zurücklesen: {e:?}"))?,
                Err(TryRecvError::Empty) => {
                    gpu.device
                        .poll(wgpu::PollType::wait_indefinitely())
                        .map_err(|e| anyhow!("auf die GPU warten: {e:?}"))?;
                }
                Err(TryRecvError::Disconnected) => bail!("die GPU hat die Kacheln nicht geliefert"),
            }
        }
        let images = {
            let view = slice
                .get_mapped_range()
                .map_err(|e| anyhow!("Kacheln zurücklesen: {e:?}"))?;
            (0..lists.len())
                .map(|k| {
                    let start = k * tile_bytes as usize;
                    RgbaImage::from_raw(
                        self.size,
                        self.size,
                        view[start..][..tile_bytes as usize].to_vec(),
                    )
                    .expect("Kachelgrösse passt")
                })
                .collect()
        };
        self.readback.unmap();
        Ok(images)
    }
}
