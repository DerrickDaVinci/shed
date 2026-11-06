// Browser preview only: when the page is opened outside the app (no
// window.__TAURI__), fake the backend with sample data so the UI can be
// designed and screenshotted in any browser. Inside the app it does nothing.
(() => {
  if (window.__TAURI__) return;
  const home = '/Users/me';
  const P = `${home}/Projects`;
  const item = (project, rel, kind, mb, extra = {}) => ({
    name: rel.split('/').pop(), kind, project: `${P}/${project}`, paths: [`${P}/${project}/${rel}`], bytes: Math.round(mb * 1e6),
    selected: true, blocked: null, notes: [], busy: [], ...extra,
  });
  const items = [
    item('weather-bot', 'src-tauri/target', 'Rust', 4180, { notes: ['ignored'] }),
    item('weather-bot', 'node_modules', 'Node', 412, { notes: ['ignored'] }),
    item('landing-page', 'node_modules', 'Node', 538, { notes: ['ignored', 'worktree'], paths: [`${P}/landing-page/.claude/worktrees/hero/node_modules`] }),
    item('landing-page', '.next', 'Node', 166, { notes: ['ignored', 'recent'] }),
    item('landing-page', 'node_modules', 'Node', 497, { notes: ['ignored'] }),
    item('landing-page', 'dist', 'Node', 74, { selected: false, notes: ['maybe'] }),
    item('stock-notes', '.venv', 'venv', 231),
    item('stock-notes', '__pycache__', 'Python', 7.6, { paths: [`${P}/stock-notes/src/__pycache__`, `${P}/stock-notes/tests/__pycache__`, `${P}/stock-notes/tools/__pycache__`] }),
    item('esp32-clock', '.pio', 'PlatformIO', 30),
    item('esp32-clock', '.venv', 'venv', 34, { selected: false, notes: ['noManifest'] }),
    item('site-archive', 'build', 'Node', 12, { selected: false, blocked: 'tracked' }),
    item('dashboard', 'node_modules', 'Node', 284, { selected: false, notes: ['ignored', 'busy'], busy: ['node'] }),
  ].sort((a, b) => b.bytes - a.bytes);
  const left = [
    { id: 'claudeVmArchive', group: 'agent', paths: [`${home}/Library/Application Support/Claude/vm_bundles/claudevm.bundle/rootfs.img.zst`], bytes: 1.25e9, selected: false, blocked: null },
    { id: 'codexInstallTemp', group: 'agent', paths: [`${home}/.cache/codex-runtimes/codex-runtime-install-x1`], bytes: 338e6, selected: true, blocked: null },
    { id: 'claudeCodeOld', group: 'agent', paths: [`${home}/Library/Application Support/Claude/claude-code/2.1.309`], bytes: 276e6, selected: true, blocked: null },
    { id: 'cargo', group: 'cache', paths: [`${home}/.cargo/registry/cache`, `${home}/.cargo/registry/src`], bytes: 979e6, selected: false, blocked: null },
    { id: 'npm', group: 'cache', paths: [`${home}/.npm/_cacache`], bytes: 643e6, selected: false, blocked: null },
    { id: 'codexSessions', group: 'history', paths: [`${home}/.codex/sessions`], bytes: 5.6e9, selected: false, blocked: 'keep' },
    { id: 'codexScratch', group: 'history', paths: [`${home}/Documents/Codex`], bytes: 1.1e9, selected: false, blocked: 'keep' },
    { id: 'claudeSessions', group: 'history', paths: [`${home}/.claude/projects`], bytes: 847e6, selected: false, blocked: 'keep' },
  ];
  // #scan,zh,dark in the address opens straight on a scan, in Chinese, dark (for screenshots).
  const opt = (k) => location.hash.slice(1).split(',').includes(k);
  let settings = { lang: opt('zh') ? 'zh' : opt('en') ? 'en' : 'auto', theme: opt('dark') ? 'dark' : opt('light') ? 'light' : 'auto', accent: '', recent: [`${home}/Desktop`, `${home}/code/experiments`], contextMenu: true };
  const history = [];
  let scanned = false;
  const wait = (ms) => new Promise((r) => setTimeout(r, ms));
  const handlers = {
    info: () => ({ version: '0.1.0', platform: 'macos', home, settings, places: [P, `${home}/Desktop`, `${home}/Documents`] }),
    set_settings: (a) => (settings = a.settings),
    scan: async (a) => (await wait(500), { roots: a.roots, refused: [], items: items.map((x) => ({ ...x })) }),
    clean: async (a) => {
      await wait(700);
      const rec = { time: Date.now(), root: P, done: a.ids.map((i) => ({ path: items[i].paths[0], trash: null, bytes: items[i].bytes })), failed: [], undone: false };
      history.unshift(rec);
      return rec;
    },
    leftovers: async () => (await wait(400), left.map((x) => ({ ...x }))),
    clean_leftovers: async (a) => {
      const rec = { time: Date.now(), root: '', done: left.filter((l) => a.ids.includes(l.id)).map((l) => ({ path: l.paths[0], bytes: l.bytes })), failed: [], undone: false };
      history.unshift(rec);
      return rec;
    },
    history: () => history,
    undo: (a) => {
      const r = history.find((x) => x.time === a.time);
      r.undone = true;
      return { ...r, failed: [] };
    },
    take_pending: () => (opt('scan') && !scanned ? ((scanned = true), [P]) : []),
    pick_folder: () => P,
    reveal: () => {},
    open_trash: () => {},
  };
  window.__TAURI__ = {
    core: { invoke: async (cmd, args) => handlers[cmd](args || {}) },
    event: { listen: () => {} },
    webview: { getCurrentWebview: () => ({ onDragDropEvent: () => {} }) },
  };
})();
