//! マスク処理をバックグラウンドスレッドで行う。古い要求は捨て、最新の要求だけ処理する。

use std::collections::HashSet;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui;
use sumiveil_core::align::{align, Row};
use sumiveil_core::{Engine, MaskResult, MaskSession};

pub struct Job {
    pub gen: u64,
    pub text: Arc<String>,
    pub engine: Arc<Engine>,
    pub excluded: Arc<HashSet<String>>,
}

pub struct Done {
    pub gen: u64,
    pub text: Arc<String>,
    pub result: MaskResult,
    /// 左の行番号 → 右の行番号
    pub left_to_right: Vec<usize>,
    /// 右の行番号 → 左の行番号
    pub right_to_left: Vec<usize>,
    pub elapsed: Duration,
}

pub struct Worker {
    tx: Sender<Job>,
    rx: Receiver<Done>,
}

fn line_maps(rows: &[Row], left_lines: usize, right_lines: usize) -> (Vec<usize>, Vec<usize>) {
    let mut l2r = vec![0usize; left_lines.max(1)];
    let mut r2l = vec![0usize; right_lines.max(1)];
    let (mut last_l, mut last_r) = (0usize, 0usize);
    for row in rows {
        if let Some(r) = row.right {
            last_r = r;
        }
        if let Some(l) = row.left {
            last_l = l;
        }
        if let Some(l) = row.left {
            if l < l2r.len() {
                l2r[l] = last_r;
            }
        }
        if let Some(r) = row.right {
            if r < r2l.len() {
                r2l[r] = last_l;
            }
        }
    }
    (l2r, r2l)
}

impl Worker {
    pub fn new(ctx: egui::Context) -> Self {
        let (tx, job_rx) = channel::<Job>();
        let (done_tx, rx) = channel::<Done>();
        std::thread::Builder::new()
            .name("sumiveil-worker".into())
            .spawn(move || {
                while let Ok(mut job) = job_rx.recv() {
                    while let Ok(newer) = job_rx.try_recv() {
                        job = newer;
                    }
                    let t = Instant::now();
                    let dets = job.engine.detect_excluding(&job.text, Some(&job.excluded));
                    let result = job.engine.apply(&job.text, &dets, &mut MaskSession::new());
                    let rows = align(&job.text, &result);
                    let ll = job.text.matches('\n').count() + 1;
                    let rl = result.output.matches('\n').count() + 1;
                    let (left_to_right, right_to_left) = line_maps(&rows, ll, rl);
                    let done = Done { gen: job.gen, text: job.text, result, left_to_right, right_to_left, elapsed: t.elapsed() };
                    if done_tx.send(done).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            })
            .expect("spawn worker");
        Self { tx, rx }
    }

    pub fn submit(&self, job: Job) {
        let _ = self.tx.send(job);
    }

    /// 完了した結果のうち最新のもの。
    pub fn poll(&self) -> Option<Done> {
        let mut last = None;
        while let Ok(d) = self.rx.try_recv() {
            last = Some(d);
        }
        last
    }
}
