//! Virtual sinks/sources owned by the daemon (they vanish with our connection).
use pipewire as pw;
use pw::properties::properties;

pub fn create_null_sink(
    core: &pw::core::CoreRc,
    name: &str,
    description: &str,
    media_class: &str,
) -> Result<pw::node::Node, pw::Error> {
    core.create_object::<pw::node::Node>(
        "adapter",
        &properties! {
            "factory.name" => "support.null-audio-sink",
            "node.name" => name,
            "node.description" => description,
            "media.class" => media_class,
            "audio.position" => "FL,FR",
            "object.linger" => "false",
            // Keep WirePlumber from picking our nodes as default devices on its own.
            "priority.session" => "0",
        },
    )
}
