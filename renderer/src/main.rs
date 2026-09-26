mod cli;

/// Der Windows-Heap nimmt beim Dekodieren von Chunks aus 24 Threads eine
/// Sperre nach der anderen; ein Chunk brauchte parallel sechsmal so lang
/// wie allein. mimalloc hat je Thread seinen eigenen Heap.
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() -> anyhow::Result<()> {
    cli::run()
}
