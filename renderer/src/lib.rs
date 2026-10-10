//! Isometrischer Offline-Renderer für Minecraft-Java-Welten: die Welt
//! lesen (`world`), das Resourcepack auflösen (`assets`), Sprites, Kacheln
//! und Zoomstufen rendern (`render`), die Entwürfe der Ebenen lesen
//! (`ebenen`).
//! Siehe docs/entwicklung/aufbau.md.

pub mod assets;
pub mod ebenen;
pub mod render;
pub mod world;
