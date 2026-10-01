use crate::error::AppError;
use std::{fs::File, io::BufReader, path::PathBuf, sync::mpsc::{self, Receiver, Sender}, thread, time::Duration};
use rodio::{Decoder, DeviceSinkBuilder, Player, Source};

#[derive(Clone)]
pub struct AudioController { tx: Sender<AudioCommand> }

enum AudioCommand {
    Play { path: Option<PathBuf>, volume: f32, looped: bool },
    Stop,
}

impl AudioController {
    pub fn start() -> Self {
        let (tx, rx) = mpsc::channel::<AudioCommand>();
        thread::spawn(move || audio_thread(rx));
        Self { tx }
    }

    pub fn play(&self, path: Option<PathBuf>, volume: f32, looped: bool) -> Result<(), AppError> {
        self.tx.send(AudioCommand::Play { path, volume: volume.clamp(0.0, 1.0), looped }).map_err(|e| AppError::Storage(e.to_string()))
    }

    pub fn stop(&self) -> Result<(), AppError> {
        self.tx.send(AudioCommand::Stop).map_err(|e| AppError::Storage(e.to_string()))
    }
}

fn audio_thread(rx: Receiver<AudioCommand>) {
    let stream = match DeviceSinkBuilder::open_default_sink() {
        Ok(stream) => stream,
        Err(error) => {
            log::warn!("Could not open default audio output: {error}");
            return;
        }
    };
    let mut player: Option<Player> = None;

    while let Ok(command) = rx.recv() {
        match command {
            AudioCommand::Play { path, volume, looped } => {
                if let Some(current) = player.take() { current.stop(); }
                let next = Player::connect_new(stream.mixer());
                next.set_volume(volume);
                let result = if let Some(path) = path {
                    File::open(&path)
                        .map(BufReader::new)
                        .map_err(|e| e.to_string())
                        .and_then(|file| Decoder::try_from(file).map_err(|e| e.to_string()))
                        .map(|source| {
                            if looped { next.append(source.repeat_infinite()); }
                            else { next.append(source); }
                        })
                } else {
                    let tone = rodio::source::SineWave::new(880.0)
                        .take_duration(Duration::from_millis(380))
                        .amplify(0.8);
                    if looped { next.append(tone.repeat_infinite()); }
                    else { next.append(tone); }
                    Ok(())
                };
                if let Err(error) = result { log::warn!("Could not play reminder sound: {error}"); }
                else { player = Some(next); }
            }
            AudioCommand::Stop => {
                if let Some(current) = player.take() { current.stop(); }
            }
        }
    }
}
