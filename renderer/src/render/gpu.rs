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
//! Die Sprites eines Durchgangs gehen mit ihm hinauf, jedes einmal: die
//! Kacheln einer Gegend brauchen ein paar hundert, die Tabelle hat
//! zehntausende. Ein Atlas auf der Karte, den sich alle Threads teilen,
//! müsste seltener hochladen, bräuchte aber eine Sperre.
//!
//! Was die Karte nicht macht: Chunks lesen, Kandidaten suchen, Sprites
//! wählen, WebP schreiben. Das bleibt auf der CPU — die Karte ersetzt nur
//! den Blit, also rund die Hälfte der Zeit je Kachel.

use std::collections::HashMap;
use std::sync::mpsc::{TryRecvError, channel};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use image::RgbaImage;

use super::Sprite;
use super::metatile::Draw;

/// Kantenlänge der Zellen, in die eine Kachel zerlegt wird: eine
/// Arbeitsgruppe je Zelle, ein Thread je Pixel. Muss zur
/// `workgroup_size` im Shader passen.
const CELL: u32 = 16;

/// Der grösste Puffer, den ein Zeichner anlegt, weit über dem, was
/// vorkommt. Dagegen prüft `ensure` die Puffer, die mit dem Durchgang
/// wachsen: Sprites, Instanzen, Zeichenlisten. Die Kacheln eines Durchgangs
/// (`out`, `readback`) legt [`Gpu::worker`] ungeprüft an; 16 Kacheln mit
/// 256 Pixeln Kante sind 4 MB.
const PUFFER_MAX: u64 = 256 << 20;

/// So lange wartet ein Durchgang höchstens auf die Karte. Hängt sie, ohne
/// dass ein Treiber sie zurücksetzt, wartete der Thread sonst ewig, und
/// der Rückfall griffe nie. Sechzehn Kacheln brauchen auf einer Karte
/// Millisekunden, auf einem Software-Adapter unter voller Last Sekunden.
const ZEITLIMIT: Duration = Duration::from_secs(60);

/// Eine geöffnete Grafikkarte mit dem Shader. Teilen sich alle Threads;
/// jeder holt sich einen [`Worker`].
pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
    /// Grösster Puffer, den ein Zeichner anlegen darf. Darüber lehnt die
    /// Karte ab, und der Standard-Handler von wgpu bräche mit einer Panik ab.
    grenze: u64,
    /// Name und Backend, für die Ausgabe beim Start.
    pub name: String,
}

impl Gpu {
    /// Öffnet die beste Grafikkarte; `None`, wenn keine da ist. Mit
    /// `software` gilt auch ein Software-Adapter (WARP, lavapipe) — für
    /// Tests auf Rechnern ohne Karte; zum Rendern ist er langsamer als
    /// der CPU-Pfad.
    pub fn new(software: bool) -> Result<Option<Gpu>> {
        Gpu::mit_grenze(software, PUFFER_MAX)
    }

    /// Wie [`Gpu::new`], aber kein Puffer grösser als `grenze` Bytes. Liegt
    /// sie unter den Anfangspuffern, scheitert schon [`Gpu::worker`] mit
    /// einer Panik aus wgpu. Für die Tests, die an die Grenze stossen.
    #[doc(hidden)]
    pub fn mit_grenze(software: bool, grenze: u64) -> Result<Option<Gpu>> {
        let Some((adapter, info)) = adapter(software)? else {
            return Ok(None);
        };
        let name = format!("{} ({:?})", info.name, info.backend);

        let limits = adapter.limits();
        let binding = grenze
            .min(limits.max_storage_buffer_binding_size)
            .min(limits.max_buffer_size);
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

        Ok(Some(Gpu {
            device,
            queue,
            pipeline,
            layout,
            grenze: binding,
            name,
        }))
    }

    /// Zerstört das Gerät, wie es ein Treiber-Reset täte. Für die Tests des
    /// Rückfalls.
    #[doc(hidden)]
    pub fn verlieren(&self) {
        self.device.destroy();
    }

    /// Ein Zeichner mit eigenen Puffern für bis zu `tiles` quadratische
    /// Kacheln mit `size` Pixeln Kante je Durchgang, für einen Thread. Der
    /// Renderlauf legt einen je Stapel an: das kostet rund 10 µs, der erste
    /// Durchgang knapp 1 ms mehr, bis die Puffer stehen.
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
        let mut worker = Worker {
            gpu: self,
            tiles,
            size,
            cells_x,
            sprites: make("sprites", 64 << 10, storage),
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
            sprite_bytes: Vec::new(),
            inst_bytes: Vec::new(),
            list_data: Vec::new(),
        };
        worker.bind = Some(worker.bind_group());
        worker
    }
}

/// Sucht den Adapter: Vulkan zuerst, derselbe Treiberweg auf Windows und
/// Linux, und mit Mesa auf dem Server ohnehin der einzige. DX12 nur, wenn
/// keine echte Karte Vulkan kann: eine alte Onboard-Grafik ohne
/// Vulkan-Treiber, WARP in der Windows-CI. GL baut der Renderer nicht mit,
/// der Weg lief nirgends in der CI; eine Karte nur mit GL-Treiber zeichnet
/// auf der CPU dasselbe Bild. Die Reihenfolge steht in [`rang`].
///
/// `WGPU_BACKEND` wählt die Backends, `WGPU_ADAPTER_NAME` einen Adapter
/// nach einem Teil seines Namens, wie bei wgpu üblich. Passt dazu keiner,
/// ist das ein Fehler: `--gpu auto` zeichnet dann auf der CPU und sagt
/// warum, `--gpu on` bricht ab.
fn adapter(software: bool) -> Result<Option<(wgpu::Adapter, wgpu::AdapterInfo)>> {
    let name = std::env::var("WGPU_ADAPTER_NAME")
        .ok()
        .map(|name| name.to_lowercase());
    // Erst Vulkan, nur echte Karten. Das spart die anderen Backends, wo
    // eine Karte Vulkan kann; eine Karte ohne Vulkan-Treiber findet erst der
    // zweite Versuch über DX12, dort aber vor lavapipe.
    let mut versuche = Vec::new();
    if std::env::var_os("WGPU_BACKEND").is_none() {
        let mut vulkan = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
        vulkan.backends = wgpu::Backends::VULKAN;
        versuche.push((vulkan, false));
    }
    versuche.push((
        wgpu::InstanceDescriptor::new_without_display_handle_from_env(),
        software,
    ));
    for (desc, software) in versuche {
        let instance = wgpu::Instance::new(desc);
        let bester = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()))
            .into_iter()
            .map(|adapter| (adapter.get_info(), adapter))
            .filter(|(info, _)| {
                name.as_ref()
                    .is_none_or(|name| info.name.to_lowercase().contains(name))
            })
            .filter_map(|(info, adapter)| {
                Some((
                    rang(info.device_type, info.backend, software)?,
                    info,
                    adapter,
                ))
            })
            .min_by_key(|(rang, _, _)| *rang);
        if let Some((_, info, adapter)) = bester {
            return Ok(Some((adapter, info)));
        }
    }
    match name {
        Some(name) if software => bail!("kein Adapter passt zu WGPU_ADAPTER_NAME={name}"),
        Some(name) => bail!(
            "keine Grafikkarte passt zu WGPU_ADAPTER_NAME={name}; einen Software-Adapter wie WARP nimmt nur --gpu on"
        ),
        None => Ok(None),
    }
}

/// Reihenfolge der Adapter, kleiner ist besser: eine echte Karte vor jedem
/// Software-Adapter, Vulkan vor den anderen Backends, eine eigenständige
/// Karte vor der Onboard-Grafik. Ohne `software` zählt kein
/// Software-Adapter.
fn rang(typ: wgpu::DeviceType, backend: wgpu::Backend, software: bool) -> Option<(bool, bool, u8)> {
    let staerke = match typ {
        wgpu::DeviceType::DiscreteGpu => 0,
        wgpu::DeviceType::IntegratedGpu => 1,
        wgpu::DeviceType::VirtualGpu => 2,
        wgpu::DeviceType::Other => 3,
        wgpu::DeviceType::Cpu if software => 4,
        wgpu::DeviceType::Cpu => return None,
    };
    Some((
        typ == wgpu::DeviceType::Cpu,
        backend != wgpu::Backend::Vulkan,
        staerke,
    ))
}

fn bytes(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|w| w.to_le_bytes()).collect()
}

/// Puffer eines Threads: Sprites und Zeichenlisten hinauf, fertige Kacheln
/// herunter.
pub struct Worker<'g> {
    gpu: &'g Gpu,
    tiles: u32,
    size: u32,
    cells_x: u32,
    sprites: wgpu::Buffer,
    instances: wgpu::Buffer,
    lists: wgpu::Buffer,
    params: wgpu::Buffer,
    out: wgpu::Buffer,
    readback: wgpu::Buffer,
    bind: Option<wgpu::BindGroup>,
    /// Die Pixel der Sprites eines Durchgangs, eines nach dem anderen.
    sprite_bytes: Vec<u8>,
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
                    entry(0, &self.sprites),
                    entry(1, &self.instances),
                    entry(2, &self.lists),
                    entry(3, &self.out),
                    entry(4, &self.params),
                ],
            })
    }

    /// Vergrössert die Puffer, wenn ein Durchgang mehr braucht, bis zur
    /// Grenze der Karte.
    fn ensure(&mut self, sprite_bytes: u64, inst_bytes: u64, list_bytes: u64) -> Result<()> {
        // Erst prüfen, dann vergrössern: sonst bände der Zeichner nach dem
        // Fehler einen Puffer, den es nicht mehr gibt.
        let grenze = self.gpu.grenze;
        for (need, label) in [
            (sprite_bytes, "Sprites"),
            (inst_bytes, "Instanzen"),
            (list_bytes, "Listen"),
        ] {
            if need > grenze {
                bail!(
                    "ein Durchgang braucht {:.1} MB für {label}, die Karte bindet höchstens {:.1} MB",
                    need as f64 / 1_048_576.0,
                    grenze as f64 / 1_048_576.0
                );
            }
        }
        let mut neu = false;
        for (buffer, need, label) in [
            (&mut self.sprites, sprite_bytes, "sprites"),
            (&mut self.instances, inst_bytes, "instances"),
            (&mut self.lists, list_bytes, "lists"),
        ] {
            if buffer.size() < need {
                *buffer = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(label),
                    size: need.next_power_of_two().min(grenze),
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                neu = true;
            }
        }
        if neu {
            self.bind = Some(self.bind_group());
        }
        Ok(())
    }

    /// Zeichnet je Liste eine Kachel. Höchstens so viele, wie der Zeichner
    /// angelegt wurde.
    pub fn render(&mut self, lists: &[Vec<Draw>]) -> Result<Vec<RgbaImage>> {
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

        // Sprites, Instanzen, Zellentabelle, Listen. Jedes Sprite kommt
        // einmal in den Puffer, beim ersten Draw, der es braucht; seine
        // Adresse (in Wörtern) steht in jeder Instanz, die es zeichnet.
        self.sprite_bytes.clear();
        self.inst_bytes.clear();
        let table = 2 * lists.len() * cells_per_tile;
        self.list_data.clear();
        self.list_data.resize(table, 0);
        let mut adresse: HashMap<*const Sprite, u32> = HashMap::new();
        let mut counts = vec![0u32; cells_per_tile];
        let mut spans: Vec<[usize; 4]> = Vec::new();
        let mut instances = 0usize;
        for (t, list) in lists.iter().enumerate() {
            spans.clear();
            counts.fill(0);
            let first = instances;
            for d in list {
                let (w, h) = (
                    d.sprite.image.width() as i32,
                    d.sprite.image.height() as i32,
                );
                let (x0, y0) = (d.origin.0.max(0), d.origin.1.max(0));
                let (x1, y1) = ((d.origin.0 + w).min(size), (d.origin.1 + h).min(size));
                if x0 >= x1 || y0 >= y1 {
                    continue;
                }
                let sprite_bytes = &mut self.sprite_bytes;
                let wort = *adresse
                    .entry(std::ptr::from_ref(d.sprite))
                    .or_insert_with(|| {
                        let wort = (sprite_bytes.len() / 4) as u32;
                        sprite_bytes.extend_from_slice(d.sprite.image.as_raw());
                        wort
                    });
                for word in [
                    wort,
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
        let list_bytes = bytes(&self.list_data);
        self.ensure(
            self.sprite_bytes.len() as u64,
            self.inst_bytes.len() as u64,
            list_bytes.len() as u64,
        )?;
        let gpu = self.gpu;
        gpu.queue.write_buffer(&self.sprites, 0, &self.sprite_bytes);
        gpu.queue.write_buffer(&self.instances, 0, &self.inst_bytes);
        gpu.queue.write_buffer(&self.lists, 0, &list_bytes);

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

        let (tx, rx) = channel();
        let slice = self.readback.slice(..out_bytes);
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        let warten = || {
            gpu.device
                .poll(wgpu::PollType::Wait {
                    submission_index: Some(index.clone()),
                    timeout: Some(ZEITLIMIT),
                })
                .map_err(|e| match e {
                    wgpu::PollError::Timeout => {
                        anyhow!("die Karte antwortet seit {} s nicht", ZEITLIMIT.as_secs())
                    }
                    e => anyhow!("auf die GPU warten: {e:?}"),
                })
        };
        warten()?;
        loop {
            match rx.try_recv() {
                Ok(result) => break result.map_err(|e| anyhow!("Kacheln zurücklesen: {e:?}"))?,
                Err(TryRecvError::Empty) => {
                    warten()?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use wgpu::{Backend, DeviceType};

    /// Verliert die Karte ihr Gerät, etwa bei einem Treiber-Reset, liefert
    /// `render` einen Fehler oder bricht mit einer Panik ab, gleich und
    /// ohne zu hängen. Beides fängt der Lauf und zeichnet auf der CPU
    /// weiter; ein Bild darf danach nicht mehr kommen, auch nicht von einem
    /// Zeichner, der erst danach entsteht.
    #[test]
    fn verlorenes_geraet_liefert_kein_bild() {
        let Some(gpu) = Gpu::new(true).unwrap() else {
            // Wie `common::ohne_gpu` in den Integrationstests.
            assert!(
                std::env::var_os("TERRANOVA_GPU_PFLICHT").is_none(),
                "kein GPU-Adapter, aber TERRANOVA_GPU_PFLICHT ist gesetzt"
            );
            eprintln!("kein GPU-Adapter, auch kein Software-Adapter — Test übersprungen");
            return;
        };
        let sprite = Sprite {
            image: RgbaImage::from_pixel(4, 4, image::Rgba([200, 10, 10, 255])),
            offset: (0, 0),
        };
        let liste = vec![Draw {
            sprite: &sprite,
            origin: (3, 3),
            skip: 0,
        }];
        let mut worker = gpu.worker(1, 64);
        let bild = worker.render(std::slice::from_ref(&liste)).unwrap();
        assert_eq!(bild[0].get_pixel(4, 4).0, [200, 10, 10, 255]);

        gpu.verlieren();
        let start = std::time::Instant::now();
        let danach = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            worker.render(std::slice::from_ref(&liste))
        }));
        assert!(
            !matches!(danach, Ok(Ok(_))),
            "nach dem Verlust kam ein Bild"
        );
        let neu = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            gpu.worker(1, 64).render(std::slice::from_ref(&liste))
        }));
        assert!(
            !matches!(neu, Ok(Ok(_))),
            "ein neuer Zeichner zeichnete auf dem verlorenen Gerät"
        );
        assert!(
            start.elapsed() < std::time::Duration::from_secs(10),
            "{:?} gewartet",
            start.elapsed()
        );
    }

    /// Eine echte Karte vor jedem Software-Adapter, dann Vulkan vor den
    /// anderen Backends, dann die stärkere Karte; ein Gerät, dessen Art der
    /// Treiber nicht nennt, zählt als Karte.
    #[test]
    fn reihenfolge_der_adapter() {
        let reihe = [
            (DeviceType::DiscreteGpu, Backend::Vulkan),
            (DeviceType::IntegratedGpu, Backend::Vulkan),
            (DeviceType::Other, Backend::Vulkan),
            (DeviceType::DiscreteGpu, Backend::Dx12),
            (DeviceType::Cpu, Backend::Vulkan),
            (DeviceType::Cpu, Backend::Dx12),
        ];
        let raenge: Vec<_> = reihe
            .iter()
            .map(|&(typ, backend)| rang(typ, backend, true).unwrap())
            .collect();
        assert!(raenge.is_sorted_by(|a, b| a < b), "{raenge:?}");
        assert_eq!(rang(DeviceType::Cpu, Backend::Vulkan, false), None);
        assert!(rang(DeviceType::Other, Backend::Vulkan, false).is_some());
    }
}
