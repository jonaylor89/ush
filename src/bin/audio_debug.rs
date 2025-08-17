use cpal::traits::{HostTrait, DeviceTrait};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = cpal::default_host();
    
    println!("Audio Host: {}", host.id().name());
    println!("{}", "=".repeat(50));
    
    // List input devices
    println!("\nINPUT DEVICES:");
    println!("{}", "-".repeat(30));
    
    match host.input_devices() {
        Ok(devices) => {
            for (i, device) in devices.enumerate() {
                println!("\nDevice {}: {}", i, device.name().unwrap_or("Unknown".to_string()));
                
                // Show supported input configurations
                match device.supported_input_configs() {
                    Ok(configs) => {
                        for (j, config) in configs.enumerate() {
                            println!("  Config {}: {} channels, {}-{} Hz, {:?}", 
                                   j, 
                                   config.channels(),
                                   config.min_sample_rate().0,
                                   config.max_sample_rate().0,
                                   config.sample_format());
                        }
                    }
                    Err(e) => println!("  Error getting configs: {}", e),
                }
            }
        }
        Err(e) => println!("Error listing input devices: {}", e),
    }
    
    // List output devices
    println!("\nOUTPUT DEVICES:");
    println!("{}", "-".repeat(30));
    
    match host.output_devices() {
        Ok(devices) => {
            for (i, device) in devices.enumerate() {
                println!("\nDevice {}: {}", i, device.name().unwrap_or("Unknown".to_string()));
                
                // Show supported output configurations
                match device.supported_output_configs() {
                    Ok(configs) => {
                        for (j, config) in configs.enumerate() {
                            println!("  Config {}: {} channels, {}-{} Hz, {:?}", 
                                   j, 
                                   config.channels(),
                                   config.min_sample_rate().0,
                                   config.max_sample_rate().0,
                                   config.sample_format());
                        }
                    }
                    Err(e) => println!("  Error getting configs: {}", e),
                }
            }
        }
        Err(e) => println!("Error listing output devices: {}", e),
    }
    
    // Check default devices specifically
    println!("\nDEFAULT DEVICES:");
    println!("{}", "-".repeat(30));
    
    if let Some(device) = host.default_input_device() {
        println!("Default input: {}", device.name().unwrap_or("Unknown".to_string()));
    } else {
        println!("No default input device found!");
    }
    
    if let Some(device) = host.default_output_device() {
        println!("Default output: {}", device.name().unwrap_or("Unknown".to_string()));
    } else {
        println!("No default output device found!");
    }
    
    Ok(())
}