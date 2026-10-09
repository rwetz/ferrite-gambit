//! One cancellable NDJSON TV connection; bounded buffering and reconnect backoff.
use serde_json::Value;
use std::io::{BufRead, BufReader, Read};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, SyncSender},
};
use std::time::Duration;

#[derive(Clone, Debug, Default)]
pub struct Position {
    pub game: String,
    pub players: String,
    pub fen: String,
    pub white: Option<u64>,
    pub black: Option<u64>,
}

pub fn apply(line: &str, position: &mut Position) -> Result<bool, String> {
    if line.trim().is_empty() {
        return Ok(false);
    }
    let v: Value = serde_json::from_str(line).map_err(|_| "Invalid TV event")?;
    let d = &v["d"];
    match v["t"].as_str() {
        Some("featured") => {
            let fen = d["fen"].as_str().ok_or("Missing TV position")?;
            board(fen)?;
            *position = Position {
                game: d["id"].as_str().unwrap_or_default().into(),
                fen: fen.into(),
                players: d["players"]
                    .as_array()
                    .map(|players| {
                        players
                            .iter()
                            .map(|p| {
                                format!("{} ({})", p["user"]["name"].as_str().unwrap_or("Anonymous"), p["rating"].as_u64().unwrap_or(0))
                            })
                            .collect::<Vec<_>>()
                            .join(" / ")
                    })
                    .unwrap_or_default(),
                white: d["players"]
                    .as_array()
                    .and_then(|players| players.iter().find(|p| p["color"] == "white"))
                    .and_then(|p| p["seconds"].as_u64()),
                black: d["players"]
                    .as_array()
                    .and_then(|players| players.iter().find(|p| p["color"] == "black"))
                    .and_then(|p| p["seconds"].as_u64()),
            };
        }
        Some("fen") => {
            let fen = d["fen"].as_str().ok_or("Missing TV position")?;
            board(fen)?;
            position.fen = fen.into();
            position.white = d["wc"].as_u64();
            position.black = d["bc"].as_u64();
        }
        _ => return Ok(false),
    }
    Ok(true)
}

pub fn board(fen: &str) -> Result<Vec<String>, String> {
    let placement = fen.split_whitespace().next().ok_or("Empty FEN")?;
    let ranks: Vec<_> = placement.split('/').collect();
    if ranks.len() != 8 {
        return Err("FEN requires eight ranks".into());
    }
    let mut rows = Vec::new();
    for (rank, source) in ranks.iter().enumerate() {
        let mut cells = Vec::new();
        for piece in source.chars() {
            if ('1'..='8').contains(&piece) {
                cells.extend(std::iter::repeat_n('.', piece as usize - '0' as usize));
            } else if "prnbqkPRNBQK".contains(piece) {
                cells.push(piece);
            } else {
                return Err("Invalid FEN piece".into());
            }
        }
        if cells.len() != 8 {
            return Err("FEN rank requires eight squares".into());
        }
        rows.push(format!("{} | {} |", 8 - rank, cells.iter().map(char::to_string).collect::<Vec<_>>().join(" ")));
    }
    rows.push("    a b c d e f g h".into());
    Ok(rows)
}

pub struct Stream {
    pub events: Receiver<Result<Position, String>>,
    stop: Arc<AtomicBool>,
}
impl Drop for Stream {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

pub fn start() -> Stream {
    let (tx, events) = mpsc::sync_channel(8);
    let stop = Arc::new(AtomicBool::new(false));
    let cancel = stop.clone();
    std::thread::spawn(move || {
        while !cancel.load(Ordering::Relaxed) {
            if let Err(error) = receive(&tx, &cancel) {
                let _ = tx.try_send(Err(error));
            }
            // Lichess asks clients to wait at least a minute after a 429.
            for _ in 0..600 {
                if cancel.load(Ordering::Relaxed) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    });
    Stream { events, stop }
}

fn receive(tx: &SyncSender<Result<Position, String>>, stop: &AtomicBool) -> Result<(), String> {
    let mut response = ureq::get("https://lichess.org/api/tv/feed")
        .header("Accept", "application/x-ndjson")
        .config()
        .timeout_global(None)
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_recv_body(Some(Duration::from_secs(30)))
        .build()
        .call()
        .map_err(|_| "TV disconnected; retrying in 60 seconds")?;
    let mut reader = BufReader::new(response.body_mut().as_reader());
    let mut position = Position::default();
    while !stop.load(Ordering::Relaxed) {
        let mut line = String::new();
        let count = reader.by_ref().take(64 * 1024).read_line(&mut line).map_err(|_| "TV stream interrupted; retrying in 60 seconds")?;
        if count == 0 {
            return Err("TV stream ended; retrying in 60 seconds".into());
        }
        if !line.ends_with('\n') {
            return Err("Oversized or incomplete TV event".into());
        }
        if apply(&line, &mut position)? {
            let _ = tx.try_send(Ok(position.clone()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    #[test]
    fn board_validates_ranks_and_preserves_colours() {
        assert!(board(FEN).unwrap()[0].contains("r n b q k b n r"));
        assert!(board("8/8").is_err());
        assert!(board("9/8/8/8/8/8/8/8").is_err());
    }
    #[test]
    fn heartbeat_and_move_events() {
        let mut p = Position::default();
        assert!(!apply("\n", &mut p).unwrap());
        let event = serde_json::json!({"t":"fen","d":{"fen":FEN,"wc":52,"bc":54}}).to_string();
        assert!(apply(&event, &mut p).unwrap());
        assert_eq!(p.white, Some(52));
        assert_eq!(p.fen, FEN);
        assert!(apply("bad", &mut p).is_err());
    }
}
