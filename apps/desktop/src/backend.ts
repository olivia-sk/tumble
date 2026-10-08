// The window's only door to Rust. Every call maps to a #[tauri::command]
// in src-tauri/src/lib.rs.
//
// Outside Tauri (opening the Vite dev server in a plain browser, for UI
// work) a small in-memory stand-in answers instead, so the layout can be
// checked without the native shell. It is never used in the real app.

export interface Target {
  id: string;
  name: string;
}

export interface FileInfo {
  path: string;
  name: string;
  format: string | null;
  targets: Target[];
}

export interface Inspected {
  files: FileInfo[];
  common: Target[];
}

export interface PresetInfo {
  name: string;
  description: string;
  to: string;
}

export interface EngineInfo {
  name: string;
  available: boolean;
  detail: string;
}

export interface ConvertArgs {
  job: number;
  path: string;
  to: string;
  quality: number | null;
  resize: string | null;
  preset: string | null;
  outDir: string | null;
}

export interface Backend {
  inspect(paths: string[]): Promise<Inspected>;
  presets(): Promise<PresetInfo[]>;
  engines(): Promise<EngineInfo[]>;
  convert(args: ConvertArgs): Promise<string[]>;
  cancel(job: number): Promise<void>;
  onProgress(handler: (job: number, fraction: number) => void): void;
  onDrop(handlers: { over: (on: boolean) => void; drop: (paths: string[]) => void }): void;
  pickFiles(folder: boolean): Promise<string[]>;
  pickFolder(): Promise<string | null>;
  reveal(path: string): Promise<void>;
}

const inTauri = "__TAURI_INTERNALS__" in window;

async function tauriBackend(): Promise<Backend> {
  const { invoke } = await import("@tauri-apps/api/core");
  const { listen } = await import("@tauri-apps/api/event");
  const { getCurrentWebview } = await import("@tauri-apps/api/webview");
  const { open } = await import("@tauri-apps/plugin-dialog");
  const { revealItemInDir } = await import("@tauri-apps/plugin-opener");
  const asList = (v: string | string[] | null): string[] => (v === null ? [] : Array.isArray(v) ? v : [v]);
  return {
    inspect: (paths) => invoke("inspect", { paths }),
    presets: () => invoke("presets"),
    engines: () => invoke("engines"),
    convert: (a) => invoke("convert", { ...a }),
    cancel: (job) => invoke("cancel", { job }),
    onProgress: (handler) => {
      listen<{ job: number; fraction: number }>("job-progress", (e) => handler(e.payload.job, e.payload.fraction));
    },
    onDrop: ({ over, drop }) => {
      getCurrentWebview().onDragDropEvent((e) => {
        const p = e.payload;
        if (p.type === "enter" || p.type === "over") over(true);
        else if (p.type === "leave") over(false);
        else if (p.type === "drop") {
          over(false);
          drop(p.paths);
        }
      });
    },
    pickFiles: async (folder) => asList(await open({ multiple: !folder, directory: folder })),
    pickFolder: async () => asList(await open({ directory: true }))[0] ?? null,
    reveal: (path) => revealItemInDir(path),
  };
}

function previewBackend(): Backend {
  const image = ["PNG", "WebP", "AVIF", "HEIC", "GIF", "TIFF", "ICO"].map((n) => ({ id: n.toLowerCase(), name: n }));
  const sample: Record<string, FileInfo> = {
    "holiday.jpg": { path: "C:\\Pics\\holiday.jpg", name: "holiday.jpg", format: "JPEG", targets: image },
    "scan.heic": { path: "C:\\Pics\\scan.heic", name: "scan.heic", format: "HEIC", targets: image.filter((t) => t.id !== "heic") },
    "clip.mov": { path: "C:\\Video\\clip.mov", name: "clip.mov", format: "MOV", targets: [{ id: "mp4", name: "MP4" }, { id: "gif", name: "GIF" }, { id: "png", name: "PNG" }] },
    "notes.xyz": { path: "C:\\Docs\\notes.xyz", name: "notes.xyz", format: null, targets: [] },
  };
  let progress: (job: number, f: number) => void = () => {};
  const cancelled = new Set<number>();
  return {
    inspect: async (paths) => {
      const files = (paths.length ? paths : Object.keys(sample)).map((p) => sample[p] ?? sample["holiday.jpg"]);
      const common = files[0]?.targets.filter((t) => files.every((f) => f.targets.some((x) => x.id === t.id))) ?? [];
      return { files, common };
    },
    presets: async () => [
      { name: "small-video", description: "MP4 (H.264) at 720p, CRF 28", to: "mp4" },
      { name: "voice", description: "MP3 at 96 kbit/s, mono", to: "mp3" },
      { name: "web", description: "WebP at quality 80, at most 2048 px", to: "webp" },
    ],
    engines: async () => [
      { name: "image", available: true, detail: "built in" },
      { name: "libheif", available: true, detail: "libheif 1.23.6" },
      { name: "pdfium", available: true, detail: "PDFium" },
      { name: "ffmpeg", available: true, detail: "FFmpeg 9.0.2" },
      { name: "libreoffice", available: false, detail: "soffice.exe not found; install with: winget install TheDocumentFoundation.LibreOffice" },
    ],
    convert: async (a) => {
      for (let i = 1; i <= 20; i++) {
        await new Promise((r) => setTimeout(r, 80));
        if (cancelled.has(a.job)) throw "cancelled";
        progress(a.job, i / 20);
      }
      if (a.path.endsWith(".heic") && a.to === "gif") throw "libheif (heic -> gif): example failure";
      return [a.path.replace(/\.[^.]+$/, "." + a.to)];
    },
    cancel: async (job) => void cancelled.add(job),
    onProgress: (h) => void (progress = h),
    onDrop: () => {},
    pickFiles: async () => Object.keys(sample),
    pickFolder: async () => "D:\\Converted",
    reveal: async () => {},
  };
}

export const backend: Promise<Backend> = inTauri ? tauriBackend() : Promise.resolve(previewBackend());
export const isPreview = !inTauri;
