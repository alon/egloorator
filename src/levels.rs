// return S->A level, A->S level (larger first)
pub fn get_levels(source: &String) -> (f64, f64)
{
    if source.contains("alsa_input.usb-Logitech_Inc._Logitech_USB_Headset_H340-00.analog-stereo") {
        return (-33f64, -35f64);
    }
    if source.contains("alsa_input.usb-Logitech_Logitech_USB_Headset-00.analog-mono") {
        return (-55f64, -57f64);
    }
    // TODO - better identifier. serial of card?
    if source == "pulsesrc device=alsa_input.usb-C-Media_Electronics_Inc._Microsoft_LifeChat_LX-3000-00.analog-mono.2" {
        return (-32f64, -34f64)
    }
    if source.contains("alsa_input.usb-C-Media_Electronics_Inc._Microsoft_LifeChat_LX-3000-00.analog-mono") {
        return (-54f64, -56f64)
    }
    if source == "pulsesrc device=alsa_input.usb-Microsoft_Microsoft_LifeChat_LX-4000-00.analog-stereo" {
        return (-40f64, -45f64)
    }
    if source.contains("alsa_input.usb-Generic_USB_Ear-Microphone_0000000001-00.analog-stereo") {
        return (-50f64, -52f64)
    }
    println!("matching default source");
    (-56f64, -58f64)
}


pub fn get_amplification(source: &String) -> f64
{
    let (s2a, a2s) = get_levels(source);
    let amplification =  -30f64 - s2a;

    println!("amplifying {} by {}", source, amplification);
    amplification
}

