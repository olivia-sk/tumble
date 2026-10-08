use super::*;
use crate::engine::{Engine, EngineError, NoProgress, Step};
use std::sync::Mutex;

/// Copies bytes and appends its step, or fails on request.
struct Copy {
    steps: Vec<Step>,
    fail_on: Option<Step>,
    pages: usize,
}

impl Engine for Copy {
    fn name(&self) -> &'static str {
        "copy"
    }
    fn available(&self) -> bool {
        true
    }
    fn steps(&self) -> Vec<Step> {
        self.steps.clone()
    }
    fn convert(
        &self,
        step: Step,
        input: &Path,
        output: &Path,
        _: &ConvertOptions,
        _: &dyn Progress,
        _: &CancelToken,
    ) -> Result<Vec<PathBuf>, EngineError> {
        let mut bytes = fs::read(input)?;
        bytes.extend_from_slice(format!("|{}>{}", step.from, step.to).as_bytes());
        if self.fail_on == Some(step) {
            fs::write(output, b"half")?;
            return Err(EngineError::failed("boom"));
        }
        if self.pages > 1 {
            let stem = output.file_stem().unwrap().to_string_lossy().into_owned();
            let ext = output.extension().unwrap().to_string_lossy().into_owned();
            let dir = output.parent().unwrap();
            return (1..=self.pages)
                .map(|p| {
                    let path = dir.join(format!("{stem}-p{p:03}.{ext}"));
                    fs::write(&path, &bytes)?;
                    Ok(path)
                })
                .collect();
        }
        fs::write(output, bytes)?;
        Ok(vec![output.to_path_buf()])
    }
}

fn registry(
    steps: &[(&'static str, &'static str)],
    fail_on: Option<Step>,
    pages: usize,
) -> Registry {
    let mut r = Registry::new();
    r.register(Box::new(Copy {
        steps: steps.iter().map(|&(a, b)| Step::new(a, b)).collect(),
        fail_on,
        pages,
    }));
    r
}

fn entries(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

struct Recorder(Mutex<Vec<f32>>);

impl Progress for Recorder {
    fn update(&self, fraction: f32) {
        self.0.lock().unwrap().push(fraction);
    }
}

fn run(
    r: &Registry,
    input: &Path,
    to: &'static str,
    out: &Path,
    namer: &OutputNamer,
    progress: &dyn Progress,
) -> Result<Outcome, JobError> {
    let options = ConvertOptions::default();
    let req =
        Request { input, to: FormatId(to), out_dir: out, options: &options, overwrite: false };
    convert_file(r, &req, namer, progress, &CancelToken::new())
}

#[test]
fn multi_hop_writes_final_file_and_cleans_up() {
    let dir = ScratchDir::new_in(&std::env::temp_dir(), "tumble-test-").unwrap();
    let input = dir.path().join("my photo.svg");
    fs::write(&input, b"src").unwrap();
    let r = registry(&[("svg", "png"), ("png", "bmp")], None, 1);
    let rec = Recorder(Mutex::new(Vec::new()));

    let outcome = run(&r, &input, "bmp", dir.path(), &OutputNamer::new(), &rec).unwrap();
    assert_eq!(outcome.outputs, [dir.path().join("my photo.bmp")]);
    assert_eq!(fs::read(&outcome.outputs[0]).unwrap(), b"src|svg>png|png>bmp");
    assert_eq!(entries(dir.path()), ["my photo.bmp", "my photo.svg"]);
    assert_eq!(*rec.0.lock().unwrap(), [0.0, 0.5, 0.5, 1.0]);
    assert_eq!(outcome.route.describe(&r), "svg -> png (copy) -> bmp (copy)");
}

#[test]
fn failure_leaves_nothing_behind() {
    let dir = ScratchDir::new_in(&std::env::temp_dir(), "tumble-test-").unwrap();
    let input = dir.path().join("a.png");
    fs::write(&input, b"src").unwrap();
    let r = registry(&[("png", "bmp")], Some(Step::new("png", "bmp")), 1);

    let err = run(&r, &input, "bmp", dir.path(), &OutputNamer::new(), &NoProgress).unwrap_err();
    assert!(matches!(err, JobError::Engine { engine: "copy", .. }), "{err}");
    assert_eq!(entries(dir.path()), ["a.png"]);
}

#[test]
fn multiple_outputs_are_numbered_independently() {
    let dir = ScratchDir::new_in(&std::env::temp_dir(), "tumble-test-").unwrap();
    let input = dir.path().join("report.svg");
    fs::write(&input, b"src").unwrap();
    fs::write(dir.path().join("report-p002.png"), b"old").unwrap();
    let r = registry(&[("svg", "png")], None, 3);

    let outcome = run(&r, &input, "png", dir.path(), &OutputNamer::new(), &NoProgress).unwrap();
    assert_eq!(outcome.outputs.len(), 3);
    assert_eq!(
        entries(dir.path()),
        [
            "report-p001.png",
            "report-p002 (1).png",
            "report-p002.png",
            "report-p003.png",
            "report.svg"
        ]
    );
}

#[test]
fn unsupported_inputs_are_reported_as_such() {
    let dir = ScratchDir::new_in(&std::env::temp_dir(), "tumble-test-").unwrap();
    let r = registry(&[("png", "bmp")], None, 1);
    let namer = OutputNamer::new();

    let err = run(&r, &dir.path().join("x.unknown"), "bmp", dir.path(), &namer, &NoProgress);
    assert!(err.unwrap_err().is_unsupported());
    let err = run(&r, &dir.path().join("x.jpg"), "bmp", dir.path(), &namer, &NoProgress);
    assert!(matches!(err, Err(JobError::NoRoute { .. })));
    let err = run(&r, &dir.path().join("missing.png"), "bmp", dir.path(), &namer, &NoProgress);
    assert!(matches!(err, Err(JobError::Io { .. })));
}

#[test]
fn cancelled_before_start_writes_nothing() {
    let dir = ScratchDir::new_in(&std::env::temp_dir(), "tumble-test-").unwrap();
    let input = dir.path().join("a.png");
    fs::write(&input, b"src").unwrap();
    let r = registry(&[("png", "bmp")], None, 1);
    let cancel = CancelToken::new();
    cancel.cancel();
    let options = ConvertOptions::default();
    let req = Request {
        input: &input,
        to: FormatId("bmp"),
        out_dir: dir.path(),
        options: &options,
        overwrite: false,
    };
    let err = convert_file(&r, &req, &OutputNamer::new(), &NoProgress, &cancel);
    assert!(matches!(err, Err(JobError::Cancelled)));
    assert_eq!(entries(dir.path()), ["a.png"]);
}
