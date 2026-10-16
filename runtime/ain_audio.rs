// Portable PCM WAV synthesizer for Ain: sequenced sine tones (16-bit mono).
// Uses only Rust's standard library and is linked only when requested.
fn ain_wavwrite(path: String, spec: AinData) -> AinData {
    let run = (|| -> Result<AinData, String> {
        if !ain_permission_allowed("write") { return Err("Ain capability denied: write".into()); }
        let AinData::Object(config) = spec else { return Err("WAV spec must be a record".into()); };
        let rate = match config.get("rate") {
            None => 44100,
            Some(AinData::Number(n)) => n.parse::<u32>().map_err(|_| "Bad WAV sample rate")?,
            _ => return Err("WAV rate must be numeric".into())
        };
        if !(8000..=96000).contains(&rate) { return Err("WAV sample rate out of range".into()); }
        let AinData::List(notes) = config.get("notes").ok_or("WAV requires notes list")?
            else { return Err("WAV notes must be a list".into()); };
        if notes.is_empty() || notes.len() > 500 { return Err("WAV requires 1..500 notes".into()); }
        let mut pcm = Vec::<i16>::new();
        for note in notes {
            let AinData::Object(values) = note else { return Err("WAV notes must be records".into()); };
            let num = |name: &str, default: i64| -> Result<i64, String> {
                match values.get(name) {
                    None => Ok(default),
                    Some(AinData::Number(n)) => n.parse::<i64>()
                        .map_err(|_| format!("Invalid numeric note field {name}")),
                    _ => Err(format!("Note field {name} must be numeric"))
                }
            };
            let hz = num("hz", 440)?;
            let ms = num("ms", 250)?;
            let volume = num("volume", 65)?;
            if !(0..=12000).contains(&hz) || !(1..=5000).contains(&ms)
                || !(0..=100).contains(&volume) {
                return Err("Note frequency, duration or volume out of range".into());
            }
            let count = u64::from(rate) * ms as u64 / 1000;
            let count = usize::try_from(count).map_err(|_| "WAV duration overflow")?;
            if pcm.len() + count > rate as usize * 15 {
                return Err("WAV total duration exceeds 15 seconds".into());
            }
            for i in 0..count {
                let phase = std::f64::consts::TAU * hz as f64 * (i as f64 / rate as f64);
                // A short fade at the start/end reduces click artefacts.
                let fade_samples = usize::min(count / 2, rate as usize / 200);
                let envelope = if fade_samples == 0 { 1.0 } else {
                    let leading = i as f64 / fade_samples as f64;
                    let trailing = (count - 1 - i) as f64 / fade_samples as f64;
                    leading.min(trailing).min(1.0)
                };
                let signal = phase.sin() * envelope * (volume as f64 / 100.0);
                pcm.push((signal * i16::MAX as f64).round() as i16);
            }
        }
        let data_size = (pcm.len() * 2) as u32;
        let mut wav = Vec::with_capacity(44 + data_size as usize);
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_size).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
        wav.extend_from_slice(&1u16.to_le_bytes()); // mono
        wav.extend_from_slice(&rate.to_le_bytes());
        wav.extend_from_slice(&(rate * 2).to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_size.to_le_bytes());
        for sample in &pcm { wav.extend_from_slice(&sample.to_le_bytes()); }
        std::fs::write(path, &wav).map_err(|e| e.to_string())?;
        Ok(AinData::Number(pcm.len().to_string()))
    })();
    match run { Ok(v) => ain_result(true, v, ""), Err(e) => ain_result(false, AinData::Null, &e) }
}
