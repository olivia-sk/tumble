// Tumble's window: a queue of files, a target picker and options.
// File names come from disk and are only ever set as text, never as HTML.

import { backend, isPreview, type Backend, type FileInfo, type PresetInfo, type Target } from "./backend";

type Status = "queued" | "running" | "done" | "failed" | "cancelled" | "unsupported";

interface Item {
  job: number;
  file: FileInfo;
  /// A per-file choice; null follows the toolbar's "Convert to".
  target: string | null;
  status: Status;
  fraction: number;
  outputs: string[];
  error: string;
}

const PARALLEL = 2;

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const el = <K extends keyof HTMLElementTagNameMap>(tag: K, cls?: string, text?: string) => {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
};

const state = {
  items: [] as Item[],
  common: [] as Target[],
  presets: [] as PresetInfo[],
  globalTarget: "",
  quality: null as number | null,
  outDir: null as string | null,
  nextJob: 1,
};

let api: Backend;

function effectiveTarget(item: Item): string {
  if (item.target) return item.target;
  return item.file.targets.some((t) => t.id === state.globalTarget) ? state.globalTarget : "";
}

function recomputeCommon() {
  const pending = state.items.filter((i) => i.status !== "unsupported");
  const first = pending[0];
  state.common = first
    ? first.file.targets.filter((t) => pending.every((i) => i.file.targets.some((x) => x.id === t.id)))
    : [];
  if (!state.common.some((t) => t.id === state.globalTarget)) {
    state.globalTarget = state.common[0]?.id ?? "";
  }
}

async function add(paths: string[]) {
  if (!paths.length) return;
  const { files } = await api.inspect(paths);
  const known = new Set(state.items.map((i) => i.file.path.toLowerCase()));
  for (const file of files) {
    if (known.has(file.path.toLowerCase())) continue;
    state.items.push({
      job: state.nextJob++,
      file,
      target: null,
      status: file.targets.length ? "queued" : "unsupported",
      fraction: 0,
      outputs: [],
      error: file.format ? `No conversions available for ${file.format}` : "Unsupported file type",
    });
  }
  recomputeCommon();
  render();
}

async function runOne(item: Item) {
  item.status = "running";
  item.fraction = 0;
  render();
  const resize = ($<HTMLInputElement>("resize").value || "").trim();
  try {
    item.outputs = await api.convert({
      job: item.job,
      path: item.file.path,
      to: effectiveTarget(item),
      quality: state.quality,
      resize: resize || null,
      preset: $<HTMLSelectElement>("preset").value || null,
      outDir: state.outDir,
    });
    item.status = "done";
  } catch (e) {
    const msg = String(e);
    item.status = msg === "cancelled" ? "cancelled" : "failed";
    item.error = msg === "cancelled" ? "Cancelled" : msg;
  }
  render();
}

async function convertAll() {
  const todo = state.items.filter((i) => (i.status === "queued" || i.status === "failed" || i.status === "cancelled") && effectiveTarget(i));
  todo.forEach((i) => {
    i.status = "queued";
    i.error = "";
  });
  render();
  let next = 0;
  const worker = async () => {
    while (next < todo.length) {
      await runOne(todo[next++]);
    }
  };
  await Promise.all(Array.from({ length: Math.min(PARALLEL, todo.length) }, worker));
}

function statusCell(item: Item): HTMLElement {
  const cell = el("div", "state");
  switch (item.status) {
    case "running": {
      const bar = el("div", "bar");
      const fill = el("div", "fill");
      fill.style.width = `${Math.round(item.fraction * 100)}%`;
      bar.append(fill);
      bar.setAttribute("role", "progressbar");
      bar.setAttribute("aria-valuenow", String(Math.round(item.fraction * 100)));
      cell.append(bar);
      break;
    }
    case "done": {
      const n = item.outputs.length;
      cell.append(el("span", "ok", n > 1 ? `Done, ${n} files` : "Done"));
      const show = el("button", "link", "Show");
      show.type = "button";
      show.onclick = () => api.reveal(item.outputs[0]);
      cell.append(show);
      break;
    }
    case "failed":
    case "unsupported":
    case "cancelled":
      cell.append(el("span", item.status === "cancelled" ? "muted" : "err", item.error));
      cell.title = item.error;
      break;
    default:
      cell.append(el("span", "muted", "Waiting"));
  }
  return cell;
}

function row(item: Item): HTMLLIElement {
  const li = el("li", `item ${item.status}`);
  const name = el("div", "name");
  name.append(el("span", "file", item.file.name), el("span", "tag", item.file.format ?? "?"));
  name.title = item.file.path;

  const pick = el("select", "row-target") as HTMLSelectElement;
  pick.setAttribute("aria-label", `Convert ${item.file.name} to`);
  pick.disabled = item.status === "running" || item.status === "unsupported";
  if (!item.file.targets.length) pick.append(new Option("—", ""));
  for (const t of item.file.targets) pick.append(new Option(t.name, t.id));
  pick.value = effectiveTarget(item);
  pick.onchange = () => {
    item.target = pick.value === state.globalTarget ? null : pick.value;
  };

  const act = el("button", "icon") as HTMLButtonElement;
  act.type = "button";
  if (item.status === "running") {
    act.textContent = "Cancel";
    act.onclick = () => api.cancel(item.job);
  } else {
    act.textContent = "×";
    act.setAttribute("aria-label", `Remove ${item.file.name}`);
    act.onclick = () => {
      state.items = state.items.filter((i) => i !== item);
      recomputeCommon();
      render();
    };
  }
  li.append(name, el("span", "arrow", "→"), pick, statusCell(item), act);
  return li;
}

function render() {
  const has = state.items.length > 0;
  $("toolbar").hidden = !has;
  $("drop").classList.toggle("compact", has);

  const target = $<HTMLSelectElement>("target");
  target.replaceChildren();
  if (!state.common.length) target.append(new Option(has ? "Choose per file" : "—", ""));
  for (const t of state.common) target.append(new Option(t.name, t.id));
  target.value = state.globalTarget;
  target.disabled = !state.common.length;

  $("queue").replaceChildren(...state.items.map(row));

  const running = state.items.some((i) => i.status === "running");
  const ready = state.items.some((i) => (i.status === "queued" || i.status === "failed" || i.status === "cancelled") && effectiveTarget(i));
  $<HTMLButtonElement>("convert").disabled = running || !ready;
  $<HTMLButtonElement>("convert").textContent = running ? "Converting…" : "Convert";

  const count = (s: Status) => state.items.filter((i) => i.status === s).length;
  const parts = [`${state.items.length} ${state.items.length === 1 ? "file" : "files"}`];
  if (count("done")) parts.push(`${count("done")} done`);
  if (count("failed")) parts.push(`${count("failed")} failed`);
  if (count("unsupported")) parts.push(`${count("unsupported")} unsupported`);
  $("status").textContent = has ? parts.join(" · ") : isPreview ? "Preview mode (no native shell)" : "";
}

async function renderEngines() {
  const names: Record<string, string> = { image: "Images", libheif: "HEIC", pdfium: "PDF", ffmpeg: "Video & audio", libreoffice: "Documents" };
  const list = $("engines");
  list.replaceChildren(
    ...(await api.engines()).map((e) => {
      const li = el("li", e.available ? "on" : "off", names[e.name] ?? e.name);
      li.title = e.detail;
      return li;
    }),
  );
}

async function main() {
  api = await backend;
  state.presets = await api.presets();
  const presetSel = $<HTMLSelectElement>("preset");
  for (const p of state.presets) presetSel.append(new Option(`${p.name} (${p.description})`, p.name));
  presetSel.onchange = () => {
    const p = state.presets.find((x) => x.name === presetSel.value);
    if (p && state.common.some((t) => t.id === p.to)) state.globalTarget = p.to;
    render();
  };

  $<HTMLSelectElement>("target").onchange = (e) => {
    state.globalTarget = (e.target as HTMLSelectElement).value;
    render();
  };
  const q = $<HTMLInputElement>("quality");
  q.oninput = () => {
    state.quality = Number(q.value);
    $("quality-value").textContent = q.value;
  };
  q.ondblclick = () => {
    state.quality = null;
    $("quality-value").textContent = "auto";
  };
  $("out-dir").onclick = async () => {
    if (state.outDir) {
      state.outDir = null;
    } else {
      state.outDir = await api.pickFolder();
    }
    $("out-dir").textContent = state.outDir ?? "Next to originals";
    $("out-dir").title = state.outDir ? "Click to go back to saving next to the originals" : "Click to choose a folder";
  };
  $("add-files").onclick = async () => add(await api.pickFiles(false));
  $("add-folder").onclick = async () => add(await api.pickFiles(true));
  $("convert").onclick = () => convertAll();
  $("clear").onclick = () => {
    state.items = state.items.filter((i) => i.status !== "done" && i.status !== "unsupported");
    recomputeCommon();
    render();
  };

  api.onProgress((job, fraction) => {
    const item = state.items.find((i) => i.job === job);
    if (item && item.status === "running") {
      item.fraction = fraction;
      const fill = document.querySelector<HTMLElement>(`#queue li:nth-child(${state.items.indexOf(item) + 1}) .fill`);
      if (fill) fill.style.width = `${Math.round(fraction * 100)}%`;
    }
  });
  api.onDrop({ over: (on) => ($("overlay").hidden = !on), drop: (paths) => add(paths) });

  render();
  await renderEngines();
  if (isPreview) await add([]);
}

main();
