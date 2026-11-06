// Shed main window. Plain JS, no build step: the real work happens in Rust.
(() => {
  const T = window.__TAURI__;
  const invoke = (cmd, args) => T.core.invoke(cmd, args);
  const $ = (s) => document.querySelector(s);
  const $$ = (s) => Array.from(document.querySelectorAll(s));
  const esc = (s) => String(s ?? '').replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]);
  const sysZh = (navigator.language || '').toLowerCase().startsWith('zh');
  let zh = sysZh;

  // ------------------------------------------------------------ copy
  const STR = {
    tagline: ['蜕 · 清掉看不见的垃圾', 'Clean what you never see'],
    navClean: ['清理文件夹', 'Clean a folder'], navLeftovers: ['AI 遗留', 'AI leftovers'], navHistory: ['记录', 'History'], navSettings: ['设置', 'Settings'], navAbout: ['关于', 'About'],
    cleanTitle: ['清理文件夹', 'Clean a folder'],
    cleanSub: ['选一个放项目的文件夹，Shed 找出里面能重新生成的东西：依赖、构建产物、缓存。', 'Pick a folder with projects. Shed finds what your tools can rebuild: dependencies, build output, caches.'],
    pick: ['选择文件夹…', 'Choose Folder…'], rescan: ['重新扫描', 'Rescan'],
    dropTitle: ['把文件夹拖到这里', 'Drop a folder here'], dropSub: ['或者点这里选择。先列清单，你确认后才会移到废纸篓。', 'or click to choose. You see the list first; nothing moves until you confirm.'],
    tipMac: ['以后可以直接在访达里右键任意文件夹 →「快速操作」或「服务」→「用 Shed 清理」。', 'Next time, right-click any folder in Finder → Quick Actions (or Services) → Clean with Shed.'],
    tipWin: ['以后可以直接在资源管理器里右键文件夹 →「用 Shed 清理」（Windows 11 在「显示更多选项」里）。', 'Next time, right-click a folder in Explorer → Clean with Shed (on Windows 11 under Show more options).'],
    quick: ['常用位置', 'Usual places'], recent: ['最近扫描', 'Recent'],
    scanning: (p) => (zh ? `正在扫描 ${p}…` : `Scanning ${p}…`),
    found: (n, b, roots) => (zh ? `${roots} 里找到 ${n} 项可以清理，共 ${b}。` : `${n} things to clean in ${roots}, ${b} in all.`),
    nothing: (roots) => (zh ? `${roots} 里没有找到可以清理的东西。很干净。` : `Nothing to clean in ${roots}. All tidy.`),
    selected: (n, b) => (zh ? `已选 <b>${n}</b> 项 · <b>${b}</b>` : `<b>${n}</b> selected · <b>${b}</b>`),
    toTrash: (b) => (zh ? `移到废纸篓 · ${b}` : `Move to Trash · ${b}`),
    r_system: ['这是系统或上层文件夹，Shed 不会扫描：', 'A system or parent folder; Shed does not scan it: '],
    r_library: ['这是应用数据文件夹，Shed 不会扫描（请用「AI 遗留」）：', 'An app data folder; Shed does not scan it (see AI leftovers): '],
    r_missing: ['文件夹不存在：', 'Folder not found: '],
    n_ignored: ['已被 .gitignore 忽略', 'ignored by git'],
    n_maybe: ['这个名字也可能是你的成果，确认后再勾选', 'a name real work can have too; tick it if you are sure'],
    n_recent: ['今天用过，清掉后下次要重新构建', 'used today; the next build will redo it'],
    n_busy: (names) => (zh ? `有程序正在这个项目里运行（${names}），先关掉它` : `${names} is running in this project; close it first`),
    n_noManifest: ['没找到依赖清单，删掉后可能装不回来', 'no dependency list found; it may be hard to recreate'],
    n_worktree: ['在 AI 代理的 worktree 里', 'inside an AI agent worktree'],
    n_tracked: ['里面有 git 跟踪的文件，不能清理', 'git tracks files in it; cannot be cleaned'],
    places: (n) => (zh ? `${n} 处` : `${n} places`),
    reveal: ['在访达中显示', 'Show in Finder'],
    // leftovers
    leftSub: ['AI 编程工具和包管理器在项目之外留下的东西。对话记录只显示占用，Shed 不会动。', 'What AI coding tools and package managers leave outside your projects. Conversation history is only shown; Shed never touches it.'],
    recheck: ['重新检查', 'Check again'], measuring: ['正在测量…', 'Measuring…'], leftEmpty: ['没有找到 AI 工具遗留或缓存。', 'No AI tool leftovers or caches found.'],
    g_agent: ['AI 工具遗留', 'AI tool leftovers'], g_cache: ['下载缓存', 'Download caches'], g_history: ['对话记录（只显示，不会动）', 'Conversation history (shown, never touched)'],
    g_cacheS: ['清掉后，下次安装依赖时会重新下载。', 'Downloaded again the next time you install.'],
    runClaude: ['Claude 桌面版正在运行：和它有关的项目要先退出它才能清理。', 'The Claude app is running: quit it to clean its items.'],
    runCodex: ['Codex 正在运行：和它有关的项目要先退出它才能清理。', 'Codex is running: quit it to clean its items.'],
    b_quitClaude: ['先退出 Claude 桌面版', 'Quit the Claude app first'], b_quitCodex: ['先退出 Codex', 'Quit Codex first'],
    L_claudeVmArchive: ['Claude 虚拟机安装包', 'Claude VM archive', 'Claude 桌面版解压虚拟机后留下的压缩包。只有重建虚拟机时才会用到，那时会重新下载（约 1 GB）。', 'The compressed copy left after the Claude app unpacked its VM. Only used if the VM is rebuilt, and then downloaded again (about 1 GB).'],
    L_claudeCodeOld: ['旧版 Claude Code（桌面版内置）', 'Old Claude Code builds (Claude app)', 'Claude 桌面版每次更新都会下载新的 Claude Code，旧的一直留着。保留最新版和正在用的。', 'The Claude app downloads Claude Code with every update and keeps the old ones. The newest and any in use stay.'],
    L_claudeCliOld: ['旧版 Claude Code 命令行', 'Old Claude Code CLI versions', '自动更新留下的旧版本。保留最新版和正在用的。', 'Old versions left by auto-update. The newest and the one in use stay.'],
    L_codexInstallTemp: ['Codex 运行时安装残留', 'Codex runtime install leftovers', 'Codex 下载运行时留下的临时文件夹，超过 1 小时没动。', 'Temporary folders from Codex runtime downloads, untouched for over an hour.'],
    L_xcodeDerived: ['Xcode DerivedData', 'Xcode DerivedData', 'Xcode 的构建缓存，下次构建时重新生成。', 'Xcode build caches, rebuilt on the next build.'],
    L_npm: ['npm 缓存', 'npm cache', 'npm 下载过的包。', 'Packages npm downloaded.'],
    L_pnpm: ['pnpm 包仓库', 'pnpm store', '已经装好的项目不受影响。', 'Installed projects keep working.'],
    L_yarn: ['Yarn 缓存', 'Yarn cache', 'Yarn 下载过的包。', 'Packages Yarn downloaded.'],
    L_cargo: ['Cargo 下载的 crate', 'Cargo downloads', 'Rust 依赖的下载包和解压出的源码。', 'Downloaded Rust crates and their unpacked sources.'],
    L_pip: ['pip 缓存', 'pip cache', 'pip 下载过的包。', 'Packages pip downloaded.'],
    L_uv: ['uv 缓存', 'uv cache', 'uv 下载过的包。用 uv 建的虚拟环境可能要重新同步。', 'Packages uv downloaded. Virtualenvs made by uv may need uv sync.'],
    L_gradle: ['Gradle 缓存', 'Gradle caches', 'Gradle 下载的依赖和构建缓存。', 'Dependencies and build caches Gradle downloaded.'],
    L_playwright: ['Playwright 浏览器', 'Playwright browsers', '测试用的浏览器，下次跑测试前要重新安装。', 'Browsers for tests; reinstall before the next test run.'],
    L_goBuild: ['Go 构建缓存', 'Go build cache', 'Go 编译缓存。', 'Go compile cache.'],
    L_homebrew: ['Homebrew 下载缓存', 'Homebrew downloads', '安装包的下载副本。', 'Downloaded copies of installed packages.'],
    L_cocoapods: ['CocoaPods 缓存', 'CocoaPods cache', 'CocoaPods 下载的库。', 'Libraries CocoaPods downloaded.'],
    L_codexSessions: ['Codex 对话记录', 'Codex conversations', '所有 Codex 对话都在这里。想删就在 Codex 里删除对话，让它自己整理。', 'Every Codex conversation lives here. To remove some, delete them in Codex so it stays consistent.'],
    L_claudeSessions: ['Claude Code 对话记录', 'Claude Code conversations', 'Claude Code 的会话记录，续聊和回退都靠它。', 'Claude Code session transcripts; resume and rewind need them.'],
    L_codexScratch: ['Codex 任务文件夹（文稿/Codex）', 'Codex task folders (Documents/Codex)', 'Codex 给每个任务建的文件夹，里面常常是你的成果。请自己检查。', 'Folders Codex makes for each task; they often hold your work. Check them yourself.'],
    // results
    working: (d, n) => (zh ? `正在移到废纸篓… ${d} / ${n}` : `Moving to the Trash… ${d} of ${n}`),
    doneTitle: ['已移到废纸篓', 'Moved to the Trash'],
    doneSub: ['清空废纸篓后才会真正腾出空间。在清空之前，随时可以撤销。', 'Empty the Trash to get the space back. Until then you can undo.'],
    nothingDone: ['没有移动任何东西', 'Nothing was moved'],
    e_gone: ['已经不在了', 'already gone'], e_unknown: ['不再像构建产物了（项目文件变了），没有动它', 'no longer looks like build output; left alone'],
    e_tracked: ['git 正在跟踪里面的文件，没有动它', 'git tracks files in it; left alone'],
    e_occupied: ['原位置已经有同名文件夹（可能重新安装过），没有覆盖', 'a folder of that name is back (reinstalled?); not overwritten'],
    e_emptied: ['已经从废纸篓清空，没法恢复', 'emptied from the Trash; cannot be restored'],
    openTrash: ['打开废纸篓', 'Open Trash'], undo: ['撤销', 'Undo'], done: ['完成', 'Done'],
    undoneOk: (n) => (zh ? `已放回 ${n} 个文件夹` : `Put back ${n} folder${n === 1 ? '' : 's'}`),
    undoneSome: (n, m) => (zh ? `已放回 ${n} 个，${m} 个没能放回` : `Put back ${n}, ${m} could not be`),
    // history
    historySub: ['每次清理都记在这里。在清空废纸篓之前都能撤销。', 'Every clean is listed here. Undo works until the Trash is emptied.'],
    historyEmpty: ['还没有清理过。', 'Nothing cleaned yet.'],
    leftName: ['AI 遗留和缓存', 'AI leftovers and caches'],
    folders: (n) => (zh ? `${n} 个文件夹` : `${n} folder${n === 1 ? '' : 's'}`),
    undone: ['已撤销', 'Undone'],
    // settings
    menuOn: ['右键菜单', 'Right-click menu'],
    menuOnWin: ['在资源管理器里右键文件夹时显示「用 Shed 清理」。只写入当前用户的注册表，卸载时会移除。', 'Show "Clean with Shed" when you right-click a folder in Explorer. Written for your user only and removed on uninstall.'],
    menuOnMac: ['在访达里右键文件夹 →「快速操作」或「服务」→「用 Shed 清理」。如果没看到：系统设置 → 键盘 → 键盘快捷键 → 服务 → 文件和文件夹，勾选「用 Shed 清理」。Shed 放进「应用程序」并打开过一次后才会出现。', 'Right-click a folder in Finder → Quick Actions (or Services) → Clean with Shed. Not there? System Settings → Keyboard → Keyboard Shortcuts → Services → Files and Folders, tick Clean with Shed. It appears once Shed is in Applications and has been opened.'],
    lang: ['界面语言', 'Language'], theme: ['外观', 'Appearance'], followSystem: ['跟随系统', 'System'], light: ['浅色', 'Light'], dark: ['深色', 'Dark'],
    accent: ['主题色', 'Accent color'], accentS: ['按钮和开关用的颜色。最后一个可以自己选任何颜色。', 'Used for buttons and switches. Pick any color with the last one.'],
    // about
    aboutTag: ['右键一个文件夹，清掉项目和 AI 编程工具留下、你从来看不见的东西。', 'Right-click a folder to clean what your projects and AI coding tools leave behind.'],
    aWhat: ['会清什么', 'What it cleans'],
    aWhatS: ['只清工具能重新生成的东西，而且要有对应的项目文件为证：package.json 旁的 node_modules、Cargo.toml 旁的 target、Python 虚拟环境、.next、__pycache__ 等二十多种。build、dist 这类名字只在 git 忽略它时才默认勾选。', 'Only what tools can rebuild, and only next to the project file that proves it: node_modules beside package.json, target beside Cargo.toml, Python virtualenvs, .next, __pycache__ and more. Names like build or dist are only ticked when git ignores them.'],
    aSafe: ['安全', 'Safety'],
    aSafeS: ['先列清单再动手；只移到废纸篓，不直接删除；git 跟踪的文件、正在运行的项目不会动；对话记录永远不碰；每次都能撤销。', 'A list before anything happens; folders go to the Trash, never deleted; nothing git tracks, nothing in use; conversation history is never touched; every clean can be undone.'],
    aOffline: ['离线', 'Offline'], aOfflineS: ['不联网，没有统计，没有账号。', 'No network, no telemetry, no account.'],
    aCli: ['命令行', 'Command line'],
    credit: ['识别规则参考了 kondo（MIT）', 'detection rules adapted from kondo (MIT)'],
    dropNow: ['松手开始扫描', 'Drop to scan'], dropNowS: ['任何文件夹都可以', 'Any folder works'],
  };
  const t = (k, ...a) => {
    const v = STR[k];
    if (typeof v === 'function') return v(...a);
    return v ? v[zh ? 0 : 1] : k;
  };
  const leftText = (id, desc) => {
    const v = STR['L_' + id];
    return v ? v[(desc ? 2 : 0) + (zh ? 0 : 1)] : id;
  };

  // ------------------------------------------------------------ helpers
  const S = { info: null, found: null, off: new Set(), on: new Set(), left: null, leftOff: new Set(), leftOn: new Set(), lastRecord: null, roots: [] };
  const settings = () => S.info.settings;
  const tilde = (p) => (p && S.info && p.startsWith(S.info.home) ? '~' + p.slice(S.info.home.length) : p);
  const base = (p) => p.split(/[\\/]/).filter(Boolean).pop() || p;
  const rel = (p, root) => (p.startsWith(root) ? p.slice(root.length).replace(/^[\\/]/, '') : p);
  const bytes = (b) => {
    const u = ['B', 'KB', 'MB', 'GB', 'TB'];
    let i = 0;
    while (b >= 1000 && i < u.length - 1) (b /= 1000), i++;
    return i === 0 ? `${b} B` : `${b >= 100 ? b.toFixed(0) : b.toFixed(1)} ${u[i]}`;
  };
  const KIND = { Rust: '#CE5A2B', Node: '#3C873A', Python: '#3572A5', venv: '#3572A5', Swift: '#F05138', Xcode: '#147EFB', Flutter: '#02569B', '.NET': '#7A4DD8', Unity: '#555', Gradle: '#02303A', PlatformIO: '#F5822A', Elixir: '#6E4A7E', Zig: '#EC915C', Haskell: '#5E5086' };
  const ICON = {
    reveal: '<svg viewBox="0 0 24 24"><path d="M3.5 7.5a2 2 0 0 1 2-2H10l2 2.5h6.5a2 2 0 0 1 2 2v7.5a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2z"/></svg>',
    folder: '<svg viewBox="0 0 24 24"><path d="M3.5 7.5a2 2 0 0 1 2-2H10l2 2.5h6.5a2 2 0 0 1 2 2v7.5a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2z"/></svg>',
    clock: '<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="8"/><path d="M12 8v4l2.5 2"/></svg>',
  };

  // ------------------------------------------------------------ i18n + theme
  function applyLang() {
    const l = settings().lang;
    zh = l === 'zh' ? true : l === 'en' ? false : sysZh;
    document.documentElement.lang = zh ? 'zh-CN' : 'en';
    $$('[data-i18n]').forEach((el) => (el.textContent = t(el.dataset.i18n)));
  }
  const applyTheme = () => Theme.apply(settings());
  async function save() {
    S.info.settings = await invoke('set_settings', { settings: settings() });
  }

  // ------------------------------------------------------------ clean: start
  function renderStart() {
    $('#cleanSub').textContent = t('cleanSub');
    $('#menuTip').textContent = t(S.info.platform === 'windows' ? 'tipWin' : 'tipMac');
    const chip = (p, icon) => `<button type="button" data-root="${esc(p)}" title="${esc(p)}">${icon}<span>${esc(tilde(p))}</span></button>`;
    $('#places').innerHTML = S.info.places.map((p) => chip(p, ICON.folder)).join('');
    $('#placesTitle').hidden = !S.info.places.length;
    const recent = settings().recent.filter((p) => !S.info.places.includes(p));
    $('#recent').innerHTML = recent.map((p) => chip(p, ICON.clock)).join('');
    $('#recentTitle').hidden = !recent.length;
  }

  function showStart() {
    S.found = null;
    $('#start').hidden = false;
    $('#results').hidden = true;
    $('#scanning').hidden = true;
    $('#rescan').hidden = true;
    renderStart();
  }

  // ------------------------------------------------------------ clean: scan
  async function scan(roots) {
    if (!roots.length) return;
    go('clean');
    S.roots = roots;
    S.off.clear();
    S.on.clear();
    $('#start').hidden = true;
    $('#results').hidden = true;
    $('#scanning').hidden = false;
    $('#scanningText').textContent = t('scanning', roots.map(tilde).join(', '));
    $('#cleanSub').textContent = '';
    try {
      S.found = await invoke('scan', { roots });
    } finally {
      $('#scanning').hidden = true;
    }
    S.info = await invoke('info'); // the scan updated the recent folders
    $('#results').hidden = false;
    $('#rescan').hidden = false;
    renderResults();
  }

  const ticked = (item, i) => !item.blocked && (item.selected ? !S.off.has(i) : S.on.has(i));
  const toggle = (item, i, on) => {
    if (item.blocked) return;
    if (on === undefined) on = !ticked(item, i);
    if (item.selected) on ? S.off.delete(i) : S.off.add(i);
    else on ? S.on.add(i) : S.on.delete(i);
  };

  function notes(item) {
    const all = [...(item.blocked ? [item.blocked] : []), ...item.notes];
    return all
      .map((n) => {
        const cls = n === 'tracked' ? 'block' : ['maybe', 'busy', 'noManifest'].includes(n) ? 'warn' : 'good';
        const text = n === 'busy' ? t('n_busy', item.busy.join(', ')) : t('n_' + n);
        return `<span class="why ${cls}">${esc(text)}</span>`;
      })
      .join('');
  }

  function renderResults() {
    const f = S.found;
    const roots = f.roots.map(tilde).join(', ');
    const total = f.items.reduce((a, i) => a + i.bytes, 0);
    $('#cleanSub').textContent = f.items.length ? t('found', f.items.length, bytes(total), roots) : t('nothing', roots);
    $('#refused').hidden = !f.refused.length;
    $('#refused').textContent = f.refused.map(([p, why]) => t('r_' + why) + tilde(p)).join('\n');

    // Group by project, biggest first.
    const groups = new Map();
    f.items.forEach((item, i) => {
      if (!groups.has(item.project)) groups.set(item.project, []);
      groups.get(item.project).push(i);
    });
    const size = (ids) => ids.reduce((a, i) => a + f.items[i].bytes, 0);
    const sorted = [...groups.entries()].sort((a, b) => size(b[1]) - size(a[1]));
    $('#groups').innerHTML = sorted
      .map(([project, ids]) => {
        const on = ids.filter((i) => ticked(f.items[i], i));
        const open = ids.filter((i) => !f.items[i].blocked);
        const all = open.length && on.length === open.length;
        const some = on.length && !all;
        return `<section class="proj" data-project="${esc(project)}">
          <div class="proj-head" data-group>
            <input type="checkbox" ${all ? 'checked' : ''} ${some ? 'data-mixed' : ''} ${open.length ? '' : 'disabled'} tabindex="-1">
            <div><div class="nm">${esc(base(project))}</div><div class="path">${esc(tilde(project))}</div></div>
            ${groupSize(size(on), size(ids))}
          </div>
          <ul class="items">${ids.map((i) => row(f.items[i], i, project)).join('')}</ul></section>`;
      })
      .join('');
    $$('#groups input[data-mixed]').forEach((el) => (el.indeterminate = true));
    renderBar();
  }

  // "1.2 GB / 1.3 GB" while partly ticked; one number otherwise.
  const groupSize = (on, all) =>
    on === all ? `<span class="size">${bytes(all)}</span>` : on ? `<span class="size">${bytes(on)}<span class="dim"> / ${bytes(all)}</span></span>` : `<span class="size dim">${bytes(all)}</span>`;

  function row(item, i, project) {
    const on = ticked(item, i);
    const name = rel(item.paths[0], project) || base(item.paths[0]);
    const many = item.paths.length > 1 ? `<small>${esc(t('places', item.paths.length))}</small>` : '';
    return `<li data-i="${i}" class="${item.blocked ? 'blocked' : on ? '' : 'off'}">
      <input type="checkbox" ${on ? 'checked' : ''} ${item.blocked ? 'disabled' : ''} tabindex="-1">
      <div><div class="nm">${esc(item.paths.length > 1 ? item.name : name)}${many}</div>
        <div class="meta"><span class="kind" style="--k:${KIND[item.kind] || 'var(--ink-3)'}"><i></i>${esc(item.kind === 'venv' ? (zh ? 'Python 虚拟环境' : 'Python virtualenv') : item.kind)}</span>${notes(item)}</div></div>
      <span class="size ${on ? '' : 'dim'}">${bytes(item.bytes)}</span>
      <div class="acts"><button type="button" class="icon-btn" data-reveal="${esc(item.paths[0])}" title="${esc(t('reveal'))}">${ICON.reveal}</button></div></li>`;
  }

  function renderBar() {
    const f = S.found;
    const ids = f.items.map((it, i) => i).filter((i) => ticked(f.items[i], i));
    const b = ids.reduce((a, i) => a + f.items[i].bytes, 0);
    $('#bar').hidden = !f.items.length;
    $('#barText').innerHTML = t('selected', ids.length, bytes(b));
    $('#cleanBtn').textContent = t('toTrash', bytes(b));
    $('#cleanBtn').disabled = !ids.length;
    return ids;
  }

  $('#groups').addEventListener('click', (e) => {
    const r = e.target.closest('[data-reveal]');
    if (r) return invoke('reveal', { path: r.dataset.reveal });
    const f = S.found;
    const li = e.target.closest('li[data-i]');
    if (li) {
      const i = Number(li.dataset.i);
      toggle(f.items[i], i);
    } else if (e.target.closest('[data-group]')) {
      const ids = [...e.target.closest('.proj').querySelectorAll('li[data-i]')].map((x) => Number(x.dataset.i));
      const open = ids.filter((i) => !f.items[i].blocked);
      const all = open.every((i) => ticked(f.items[i], i));
      open.forEach((i) => toggle(f.items[i], i, !all));
    } else return;
    const y = $('#main').scrollTop;
    renderResults();
    $('#main').scrollTop = y;
  });

  $('#cleanBtn').addEventListener('click', async () => {
    const ids = renderBar();
    if (!ids.length) return;
    const rec = await runClean(() => invoke('clean', { ids }));
    if (rec) scan(S.roots);
  });

  // ------------------------------------------------------------ result sheet
  async function runClean(call) {
    $('#sheet').hidden = false;
    $('#sheetTitle').textContent = t('doneTitle');
    $('#sheetSub').textContent = '';
    $('#sheetBig').hidden = true;
    $('#sheetList').innerHTML = '';
    $('#busy').hidden = false;
    $('#busyText').textContent = t('working', 0, '…');
    ['#sheetTrash', '#sheetUndo'].forEach((b) => ($(b).hidden = true));
    $('#sheetDone').disabled = true;
    let rec = null;
    try {
      rec = await call();
    } finally {
      $('#busy').hidden = true;
      $('#sheetDone').disabled = false;
    }
    S.lastRecord = rec;
    const total = rec.done.reduce((a, d) => a + d.bytes, 0);
    $('#sheetTitle').textContent = rec.done.length ? t('doneTitle') : t('nothingDone');
    $('#sheetBig').hidden = !rec.done.length;
    $('#sheetBig').textContent = bytes(total);
    $('#sheetSub').textContent = rec.done.length ? t('doneSub') : '';
    $('#sheetList').innerHTML = [
      ...rec.failed.map((x) => `<li><div><div class="nm">${esc(base(x.path))}</div><div class="route">${esc(tilde(x.path))}</div><span class="why warn">${esc(t('e_' + x.error) === 'e_' + x.error ? x.error : t('e_' + x.error))}</span></div></li>`),
      ...rec.done.map((x) => `<li><div><div class="nm">${esc(base(x.path))}</div><div class="route">${esc(tilde(x.path))}</div></div><span class="size">${bytes(x.bytes)}</span></li>`),
    ].join('');
    $('#sheetTrash').hidden = !rec.done.length;
    $('#sheetUndo').hidden = !rec.done.length;
    return rec;
  }

  $('#sheetDone').addEventListener('click', () => ($('#sheet').hidden = true));
  $('#sheetTrash').addEventListener('click', () => invoke('open_trash'));
  $('#sheetUndo').addEventListener('click', async () => {
    if (!S.lastRecord) return;
    $('#sheet').hidden = true;
    await undo(S.lastRecord.time);
  });

  async function undo(time) {
    let back;
    try {
      back = await invoke('undo', { time });
    } catch (e) {
      return toast(String(e));
    }
    toast(back.failed.length ? t('undoneSome', back.done.length, back.failed.length) + ' · ' + back.failed.map((f) => `${base(f.path)}: ${t('e_' + f.error)}`).join('; ') : t('undoneOk', back.done.length));
    if (S.found) scan(S.roots);
    if (S.left) loadLeftovers();
    renderHistory();
  }

  // ------------------------------------------------------------ leftovers
  async function loadLeftovers() {
    $('#leftBusy').hidden = false;
    $('#leftGroups').innerHTML = '';
    $('#leftBar').hidden = true;
    $('#runBanner').hidden = true;
    S.leftOff.clear();
    S.leftOn.clear();
    try {
      S.left = await invoke('leftovers');
    } finally {
      $('#leftBusy').hidden = true;
    }
    renderLeftovers();
  }

  const lticked = (l) => !l.blocked && (l.selected ? !S.leftOff.has(l.id) : S.leftOn.has(l.id));

  function renderLeftovers() {
    const L = S.left;
    const run = [L.some((l) => l.blocked === 'quitClaude') ? t('runClaude') : '', L.some((l) => l.blocked === 'quitCodex') ? t('runCodex') : ''].filter(Boolean);
    $('#runBanner').hidden = !run.length;
    $('#runBanner').textContent = run.join(' ');
    const group = (g) => {
      const items = L.filter((l) => l.group === g);
      if (!items.length) return '';
      const sum = items.reduce((a, l) => a + l.bytes, 0);
      return `<div class="group-title"><b>${esc(t('g_' + g))}</b><span>${g === 'cache' ? esc(t('g_cacheS')) + ' · ' : ''}${bytes(sum)}</span></div>
        <section class="proj"><ul class="items">${items.map(leftRow).join('')}</ul></section>`;
    };
    $('#leftGroups').innerHTML = ['agent', 'cache', 'history'].map(group).join('') || `<p class="empty">${esc(t('leftEmpty'))}</p>`;
    const on = L.filter(lticked);
    const b = on.reduce((a, l) => a + l.bytes, 0);
    $('#leftBar').hidden = !L.some((l) => l.group !== 'history');
    $('#leftBarText').innerHTML = t('selected', on.length, bytes(b));
    $('#leftBtn').textContent = t('toTrash', bytes(b));
    $('#leftBtn').disabled = !on.length;
  }

  function leftRow(l) {
    const info = l.group === 'history';
    const on = lticked(l);
    const block = l.blocked && !info ? `<span class="why block">${esc(t('b_' + l.blocked))}</span>` : '';
    const paths = l.paths.map(tilde).join(' · ');
    return `<li data-id="${esc(l.id)}" class="${info ? 'info' : l.blocked ? 'blocked' : on ? '' : 'off'}">
      ${info ? '' : `<input type="checkbox" ${on ? 'checked' : ''} ${l.blocked ? 'disabled' : ''} tabindex="-1">`}
      <div><div class="lnm">${esc(leftText(l.id))}</div><div class="desc">${esc(leftText(l.id, true))}</div>
        <div class="meta"><span class="why mono" title="${esc(paths)}">${esc(paths)}</span>${block}</div></div>
      <span class="size ${on || info ? '' : 'dim'}">${bytes(l.bytes)}</span>
      <div class="acts"><button type="button" class="icon-btn" data-reveal="${esc(l.paths[0])}" title="${esc(t('reveal'))}">${ICON.reveal}</button></div></li>`;
  }

  $('#leftGroups').addEventListener('click', (e) => {
    const r = e.target.closest('[data-reveal]');
    if (r) return invoke('reveal', { path: r.dataset.reveal });
    const li = e.target.closest('li[data-id]');
    const l = li && S.left.find((x) => x.id === li.dataset.id);
    if (!l || l.blocked) return;
    const now = !lticked(l);
    if (l.selected) now ? S.leftOff.delete(l.id) : S.leftOff.add(l.id);
    else now ? S.leftOn.add(l.id) : S.leftOn.delete(l.id);
    renderLeftovers();
  });
  $('#recheck').addEventListener('click', loadLeftovers);
  $('#leftBtn').addEventListener('click', async () => {
    const ids = S.left.filter(lticked).map((l) => l.id);
    if (!ids.length) return;
    await runClean(() => invoke('clean_leftovers', { ids }));
    loadLeftovers();
  });

  // ------------------------------------------------------------ history
  async function renderHistory() {
    const h = await invoke('history');
    $('#history').innerHTML = h.length
      ? h
          .map((r) => {
            const when = new Date(r.time).toLocaleString(zh ? 'zh-CN' : 'en-US', { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' });
            const total = r.done.reduce((a, d) => a + d.bytes, 0);
            const act = r.undone ? `<span class="when">${esc(t('undone'))}</span>` : `<button type="button" class="btn" data-undo="${r.time}">${esc(t('undo'))}</button>`;
            const name = r.root ? r.root.split(', ').map((p) => base(p)).join(', ') : t('leftName');
            return `<li class="${r.undone ? 'undone' : ''}">
              <div><div class="nm">${esc(name)} · ${bytes(total)}</div>
                <div class="route" title="${esc(r.done.map((d) => d.path).join('\n'))}">${esc(r.done.slice(0, 3).map((d) => tilde(d.path)).join(' · '))}${r.done.length > 3 ? ' …' : ''}</div>
                <div class="meta"><span class="when">${esc(when)}</span><span class="when">${esc(t('folders', r.done.length))}</span></div></div>
              <div class="acts">${act}</div></li>`;
          })
          .join('')
      : `<li><span class="empty">${esc(t('historyEmpty'))}</span></li>`;
  }
  $('#history').addEventListener('click', (e) => {
    const u = e.target.closest('[data-undo]');
    if (!u) return;
    u.disabled = true;
    undo(Number(u.dataset.undo));
  });

  // ------------------------------------------------------------ settings
  function renderSettings() {
    const s = settings();
    $$('.seg[data-set]').forEach((seg) => seg.querySelectorAll('button').forEach((b) => b.setAttribute('aria-pressed', String(b.dataset.v === String(s[seg.dataset.set])))));
    $$('input[data-set]').forEach((i) => (i.checked = !!s[i.dataset.set]));
    const preset = $$('#swatches button').some((b) => b.dataset.c === (s.accent || ''));
    $$('#swatches button').forEach((b) => b.setAttribute('aria-pressed', String(b.dataset.c === (s.accent || ''))));
    $('#swatches .custom').classList.toggle('on', !preset);
    $('#accentPick').value = s.accent || '#4A55D0';
  }
  $$('input[data-set]').forEach((i) =>
    i.addEventListener('change', async () => {
      settings()[i.dataset.set] = i.checked;
      await save();
      renderSettings();
    }),
  );
  async function setAccent(c) {
    settings().accent = c;
    applyTheme();
    renderSettings();
    await save();
  }
  $('#swatches').addEventListener('click', (e) => {
    const b = e.target.closest('button[data-c]');
    if (b) setAccent(b.dataset.c);
  });
  $('#accentPick').addEventListener('input', (e) => {
    settings().accent = e.target.value;
    applyTheme();
  });
  $('#accentPick').addEventListener('change', (e) => setAccent(e.target.value));
  $$('.seg[data-set]').forEach((seg) =>
    seg.addEventListener('click', async (e) => {
      const b = e.target.closest('button');
      if (!b) return;
      settings()[seg.dataset.set] = b.dataset.v;
      await save();
      applyLang();
      applyTheme();
      renderAll();
    }),
  );

  // ------------------------------------------------------------ shell
  function go(page) {
    $('#app').dataset.page = page;
    $$('#nav button').forEach((b) => b.setAttribute('aria-current', b.dataset.page === page ? 'page' : 'false'));
    if (page === 'history') renderHistory();
    if (page === 'leftovers' && !S.left) loadLeftovers();
  }
  $('#nav').addEventListener('click', (e) => {
    const b = e.target.closest('button[data-page]');
    if (!b) return;
    if (b.dataset.page === 'clean' && $('#app').dataset.page === 'clean' && S.found) return showStart();
    go(b.dataset.page);
  });

  const pickAndScan = async () => {
    const p = await invoke('pick_folder');
    if (p) scan([p]);
  };
  $('#pick').addEventListener('click', pickAndScan);
  $('#dropzone').addEventListener('click', pickAndScan);
  $('#rescan').addEventListener('click', () => scan(S.roots));
  $('#start').addEventListener('click', (e) => {
    const b = e.target.closest('[data-root]');
    if (b) scan([b.dataset.root]);
  });

  let toastTimer;
  function toast(text) {
    $('#toastText').textContent = text;
    $('#toast').classList.add('show');
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => $('#toast').classList.remove('show'), 6000);
  }

  function renderAll() {
    if (S.found) renderResults();
    else renderStart();
    if (S.left) renderLeftovers();
    renderSettings();
    if ($('#app').dataset.page === 'history') renderHistory();
  }

  // Folders dropped on the window are scanned.
  T.webview.getCurrentWebview().onDragDropEvent((ev) => {
    const type = ev.payload.type;
    $('#drop').hidden = !(type === 'enter' || type === 'over');
    if (type === 'drop' && ev.payload.paths?.length && $('#sheet').hidden) scan(ev.payload.paths);
  });

  // Folders from Finder or Explorer (right-click), now or at launch.
  async function takePending() {
    const paths = await invoke('take_pending');
    if (paths.length && $('#sheet').hidden) scan(paths);
  }
  T.event.listen('open-folders', takePending);
  T.event.listen('progress', (e) => ($('#busyText').textContent = t('working', ...e.payload)));

  (async () => {
    S.info = await invoke('info');
    document.documentElement.classList.add(S.info.platform === 'windows' ? 'os-win' : 'os-mac');
    if (S.info.platform === 'windows') STR.reveal = ['在资源管理器中显示', 'Show in Explorer'];
    $('#ver').textContent = 'v' + S.info.version;
    $('#aboutVer').textContent = 'v' + S.info.version;
    applyLang();
    applyTheme();
    go('clean');
    showStart();
    renderSettings();
    takePending();
  })();
})();
