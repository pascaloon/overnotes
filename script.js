/* Overnotes website — scroll-driven overlay demo (no dependencies) */
(() => {
  "use strict";

  const $ = (id) => document.getElementById(id);
  const scrolly = document.querySelector(".scrolly");
  const stage = $("stage");
  const hero = $("hero");
  const chrome = document.querySelector(".chrome");
  const backdrop = document.querySelector(".overlay-backdrop");
  const hud = document.querySelector(".hud");
  const drawOpts = document.querySelector(".draw-opts");
  const cursor = $("cursor");
  const ring = $("ring");
  const hotkeyE = $("hotkeyE");
  const hotkeyS = $("hotkeyS");
  const shotHint = $("shotHint");
  const note = $("ovNote");
  const typed = $("typed");
  const caret = document.querySelector(".caret");
  const penPath = $("penPath");
  const cropRect = $("cropRect");
  const flash = document.querySelector(".shot-flash");
  const shot1 = $("shot1");
  const shot2 = $("shot2");
  const folder = $("ovFolder");
  const moveLabel = $("moveLabel");
  const captions = [1, 2, 3, 4, 5, 6].map((i) => $("cap" + i));
  const tools = {
    select: $("toolSelect"),
    note: $("toolNote"),
    draw: $("toolDraw"),
    sub: $("toolSub"),
  };
  const btnShot = $("btnShot");

  const NOTE_TEXT = "Rune wants 3 sigils. Found 2 \u2014 is the last one on the drone's patrol route?";

  /* ---------------- helpers ---------------- */

  const clamp = (v, a, b) => Math.max(a, Math.min(b, v));
  const clamp01 = (v) => clamp(v, 0, 1);
  const seg = (p, a, b) => clamp01((p - a) / (b - a));
  const lerp = (a, b, t) => a + (b - a) * t;
  const easeIO = (t) => (t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2);
  const easeO = (t) => 1 - Math.pow(1 - t, 3);
  const easeBack = (t) => {
    const c = 1.70158;
    return 1 + (c + 1) * Math.pow(t - 1, 3) + c * Math.pow(t - 1, 2);
  };
  // fade in over the first `f`, out over the last `f` of [a, b]
  const window01 = (p, a, b, f = 0.018) =>
    Math.min(seg(p, a, a + f), 1 - seg(p, b - f, b));

  /* scene (1600x900, xMidYMid slice) -> stage pixels */
  function s2v(x, y) {
    const w = stage.clientWidth;
    const h = stage.clientHeight;
    const s = Math.max(w / 1600, h / 900);
    return {
      x: (w - 1600 * s) / 2 + x * s,
      y: (h - 900 * s) / 2 + y * s,
    };
  }

  function toolCenter(el) {
    const sr = stage.getBoundingClientRect();
    const r = el.getBoundingClientRect();
    return { x: r.left - sr.left + r.width / 2, y: r.top - sr.top + r.height / 2 };
  }

  const lerpRect = (a, b, t) => ({
    x: lerp(a.x, b.x, t),
    y: lerp(a.y, b.y, t),
    w: lerp(a.w, b.w, t),
    h: lerp(a.h, b.h, t),
  });

  function applyRect(el, r) {
    el.style.left = r.x + "px";
    el.style.top = r.y + "px";
    el.style.width = r.w + "px";
    el.style.height = r.h + "px";
  }

  /* ---------------- timeline ---------------- */

  const T = {
    hero: [0.0, 0.085],
    keyE1: [0.085, 0.15],
    keyE1press: 0.112,
    chromeIn: [0.108, 0.15],
    cap1: [0.11, 0.205],

    noteTool: [0.16, 0.176],
    noteToolClick: 0.178,
    noteMove: [0.182, 0.198],
    noteClick: 0.2,
    notePop: [0.2, 0.212],
    noteType: [0.214, 0.3],
    cap2: [0.165, 0.31],

    drawTool: [0.318, 0.334],
    drawToolClick: 0.336,
    toStroke: [0.338, 0.352],
    stroke: [0.352, 0.44],
    cap3: [0.325, 0.455],

    keyS: [0.468, 0.52],
    keySpress: 0.492,
    shotMode: [0.494, 0.58],
    crop: [0.51, 0.566],
    flash: [0.566, 0.584],
    shot1In: [0.584, 0.606],
    shot2In: [0.61, 0.634],
    cap4: [0.47, 0.64],

    subTool: [0.648, 0.664],
    subToolClick: 0.666,
    subMove: [0.668, 0.684],
    subClick: 0.686,
    folderPop: [0.686, 0.698],
    drag1: [0.702, 0.736],
    drag2: [0.742, 0.776],
    cap5: [0.65, 0.79],

    keyE2: [0.8, 0.85],
    keyE2press: 0.822,
    overview: [0.822, 0.862],
    cap6: [0.81, 0.98],
  };

  /* ---------------- layout anchors ---------------- */

  const NOTE_SCENE = { x: 1095, y: 128 };
  const CROP_A = { x: 500, y: 470 };
  const CROP_B = { x: 790, y: 860 };

  function notePos() {
    const p = s2v(NOTE_SCENE.x, NOTE_SCENE.y);
    const w = stage.clientWidth;
    const nw = note.offsetWidth || 230;
    return { x: clamp(p.x, 12, w - nw - 12), y: Math.max(p.y, 70) };
  }

  function cropArea() {
    const a = s2v(CROP_A.x, CROP_A.y);
    const b = s2v(CROP_B.x, CROP_B.y);
    const minX = Math.max(a.x, 8);
    return { x: minX, y: a.y, w: b.x - minX, h: b.y - a.y };
  }

  function shot1Rest() {
    const w = stage.clientWidth, h = stage.clientHeight;
    const sw = Math.min(190, w * 0.17);
    return { x: w * 0.55, y: h * 0.46, w: sw, h: (sw * 390) / 290 };
  }

  function shot2Rest() {
    const w = stage.clientWidth, h = stage.clientHeight;
    const sw = Math.min(215, w * 0.19);
    return { x: w * 0.73, y: h * 0.3, w: sw, h: (sw * 160) / 190 };
  }

  function folderRect() {
    const w = stage.clientWidth, h = stage.clientHeight;
    const fw = Math.min(118, Math.max(84, w * 0.085));
    return { x: w * 0.8, y: h * 0.64, w: fw, h: fw * 0.83 };
  }

  const rectCenter = (r) => ({ x: r.x + r.w / 2, y: r.y + r.h / 2 });

  function folderDropRect() {
    const f = folderRect();
    const c = rectCenter(f);
    return { x: c.x - 14, y: c.y - 18, w: 28, h: 28 };
  }

  /* ---------------- cursor track ---------------- */

  let penLen = 0;

  function penPoint(t) {
    const pt = penPath.getPointAtLength(penLen * clamp01(t));
    return s2v(pt.x, pt.y);
  }

  function buildTrack() {
    const w = stage.clientWidth, h = stage.clientHeight;
    const center = { x: w * 0.5, y: h * 0.62 };
    const np = notePos();
    const noteClickPt = { x: np.x + 40, y: np.y + 30 };
    const ca = cropArea();
    const cropStart = { x: ca.x, y: ca.y };
    const cropEnd = { x: ca.x + ca.w, y: ca.y + ca.h };
    const fCenter = rectCenter(folderRect());
    const folderClickPt = { x: fCenter.x, y: fCenter.y - 8 };
    const s1c = rectCenter(shot1Rest());
    const s2c = rectCenter(shot2Rest());

    return [
      [0.108, center],
      [T.noteTool[0], toolCenter(tools.note)],
      [T.noteToolClick + 0.004, toolCenter(tools.note)],
      [T.noteMove[1], noteClickPt],
      [T.noteType[0] + 0.02, { x: noteClickPt.x + 70, y: noteClickPt.y + 90 }],
      [T.drawTool[0], toolCenter(tools.draw)],
      [T.drawToolClick + 0.004, toolCenter(tools.draw)],
      [T.stroke[0], penPoint(0)],
      [T.stroke[1], penPoint(1)],
      [T.keyS[0], { x: w * 0.5, y: h * 0.72 }],
      [T.crop[0], cropStart],
      [T.crop[1], cropEnd],
      [T.shot1In[1], rectCenter(lerpRect(cropArea(), shot1Rest(), 1))],
      [T.subTool[0], toolCenter(tools.sub)],
      [T.subToolClick + 0.004, toolCenter(tools.sub)],
      [T.subMove[1], folderClickPt],
      [T.drag1[0], s1c],
      [T.drag1[1], fCenter],
      [T.drag2[0], s2c],
      [T.drag2[1], fCenter],
      [0.8, { x: w * 0.62, y: h * 0.7 }],
    ];
  }

  function cursorPos(p) {
    // while drawing, glue the cursor to the pen tip
    if (p >= T.stroke[0] && p <= T.stroke[1]) {
      return penPoint(easeIO(seg(p, T.stroke[0], T.stroke[1])));
    }
    const track = buildTrack();
    if (p <= track[0][0]) return track[0][1];
    for (let i = 0; i < track.length - 1; i++) {
      const [t0, p0] = track[i];
      const [t1, p1] = track[i + 1];
      if (p >= t0 && p <= t1) {
        const t = easeIO(seg(p, t0, t1));
        return { x: lerp(p0.x, p1.x, t), y: lerp(p0.y, p1.y, t) };
      }
    }
    return track[track.length - 1][1];
  }

  /* ---------------- clicks ---------------- */

  const CLICK_DUR = 0.016;

  function clickMoments() {
    return [
      { t: T.noteToolClick, pos: toolCenter(tools.note) },
      { t: T.noteClick, pos: cursorPos(T.noteClick) },
      { t: T.drawToolClick, pos: toolCenter(tools.draw) },
      { t: T.subToolClick, pos: toolCenter(tools.sub) },
      { t: T.subClick, pos: cursorPos(T.subClick) },
    ];
  }

  function updateRing(p) {
    let best = null;
    for (const c of clickMoments()) {
      if (p >= c.t && p <= c.t + CLICK_DUR) best = c;
    }
    if (!best) {
      ring.style.opacity = "0";
      return;
    }
    const t = seg(p, best.t, best.t + CLICK_DUR);
    ring.style.left = best.pos.x + "px";
    ring.style.top = best.pos.y + "px";
    ring.style.opacity = String(0.9 * (1 - t));
    ring.style.transform = `scale(${lerp(0.35, 1.25, easeO(t))})`;
  }

  /* ---------------- per-frame update ---------------- */

  function activeTool(p) {
    if (p >= T.noteToolClick && p < T.drawToolClick) return "note";
    if (p >= T.drawToolClick && p < T.keyS[0]) return "draw";
    if (p >= T.subToolClick && p < T.drag1[0]) return "sub";
    return "select";
  }

  function setHotkey(el, p, win, pressT) {
    const o = window01(p, win[0], win[1], 0.014);
    el.style.opacity = String(o);
    el.style.transform = `translateX(-50%) translateY(${lerp(10, 0, easeO(o))}px)`;
    el.classList.toggle("pressed", p >= pressT && p <= pressT + 0.02);
  }

  function update(p) {
    const w = stage.clientWidth, h = stage.clientHeight;

    /* hero */
    const heroOut = seg(p, 0.02, T.hero[1]);
    hero.style.opacity = String(1 - easeIO(heroOut));
    hero.style.pointerEvents = heroOut > 0.6 ? "none" : "auto";
    hero.style.visibility = heroOut >= 1 ? "hidden" : "visible";

    /* hotkeys */
    setHotkey(hotkeyE, p, T.keyE1, T.keyE1press);
    if (p > (T.keyE1[1] + T.keyE2[0]) / 2) {
      setHotkey(hotkeyE, p, T.keyE2, T.keyE2press);
    }
    setHotkey(hotkeyS, p, T.keyS, T.keySpress);

    /* overlay chrome + backdrop (in after first hotkey, out in overview) */
    const chromeIn = easeIO(seg(p, T.chromeIn[0], T.chromeIn[1]));
    const overviewT = easeIO(seg(p, T.overview[0], T.overview[1]));
    const chromeO = chromeIn * (1 - overviewT);
    chrome.style.opacity = String(chromeO);
    backdrop.style.opacity = String(chromeO);
    hud.style.opacity = String(1 - 0.72 * chromeO);

    /* tools */
    const tool = activeTool(p);
    for (const k in tools) tools[k].classList.toggle("active", k === tool);
    drawOpts.style.opacity = tool === "draw" ? "1" : "0";
    btnShot.classList.toggle("active", p >= T.keySpress && p <= T.shotMode[1]);

    /* captions */
    const capWins = [T.cap1, T.cap2, T.cap3, T.cap4, T.cap5, T.cap6];
    captions.forEach((el, i) => {
      const [a, b] = capWins[i];
      const o = window01(p, a, b);
      el.style.opacity = String(o);
      el.style.transform = `translateY(${lerp(18, 0, easeO(o))}px)`;
    });

    /* sticky note */
    const np = notePos();
    note.style.left = np.x + "px";
    note.style.top = np.y + "px";
    const notePopT = easeBack(seg(p, T.notePop[0], T.notePop[1]));
    const noteO = p >= T.notePop[0] ? 1 : 0;
    const noteOverview = lerp(1, 0.68, overviewT);
    note.style.opacity = String(noteO * notePopT * noteOverview);
    note.style.transform = `rotate(-2deg) scale(${lerp(0.55, 1, clamp01(notePopT))})`;

    const typeT = easeIO(seg(p, T.noteType[0], T.noteType[1]));
    typed.textContent = NOTE_TEXT.slice(0, Math.round(NOTE_TEXT.length * typeT));
    caret.style.display = p >= T.notePop[0] && p < T.noteType[1] + 0.02 ? "" : "none";

    /* pen stroke */
    const strokeT = easeIO(seg(p, T.stroke[0], T.stroke[1]));
    penPath.style.strokeDasharray = String(penLen);
    penPath.style.strokeDashoffset = String(penLen * (1 - strokeT));
    penPath.style.opacity = String((strokeT > 0 ? 1 : 0) * lerp(1, 0.75, overviewT));

    /* screenshot mode */
    const inShotMode = p >= T.shotMode[0] && p <= T.shotMode[1];
    stage.classList.toggle("shot-mode", inShotMode);
    shotHint.style.opacity = String(window01(p, T.shotMode[0], T.crop[1], 0.012));

    /* crop rect */
    const ca = cropArea();
    const cropT = easeIO(seg(p, T.crop[0], T.crop[1]));
    if (p >= T.crop[0] && p <= T.flash[0] + 0.004) {
      applyRect(cropRect, { x: ca.x, y: ca.y, w: ca.w * cropT, h: ca.h * cropT });
      cropRect.style.opacity = "1";
    } else {
      cropRect.style.opacity = "0";
    }

    /* flash */
    const ft = seg(p, T.flash[0], T.flash[1]);
    flash.style.opacity = String(ft > 0 && ft < 1 ? 0.75 * Math.pow(1 - ft, 2) : 0);

    /* shot 1: crop area -> resting spot -> dragged into folder */
    const s1rest = shot1Rest();
    const fDrop = folderDropRect();
    if (p < T.shot1In[0]) {
      shot1.style.opacity = "0";
    } else if (p < T.drag1[0]) {
      const t = easeIO(seg(p, T.shot1In[0], T.shot1In[1]));
      applyRect(shot1, lerpRect(ca, s1rest, t));
      shot1.style.opacity = "1";
    } else {
      const t = easeIO(seg(p, T.drag1[0], T.drag1[1]));
      applyRect(shot1, lerpRect(s1rest, fDrop, t));
      shot1.style.opacity = String(1 - seg(p, T.drag1[0] + (T.drag1[1] - T.drag1[0]) * 0.7, T.drag1[1]));
    }

    /* shot 2: slides in from the right -> dragged into folder */
    const s2rest = shot2Rest();
    if (p < T.shot2In[0]) {
      shot2.style.opacity = "0";
    } else if (p < T.drag2[0]) {
      const t = easeO(seg(p, T.shot2In[0], T.shot2In[1]));
      const from = { x: w + 40, y: s2rest.y - 30, w: s2rest.w, h: s2rest.h };
      applyRect(shot2, lerpRect(from, s2rest, t));
      shot2.style.opacity = String(Math.min(1, t * 2));
    } else {
      const t = easeIO(seg(p, T.drag2[0], T.drag2[1]));
      applyRect(shot2, lerpRect(s2rest, fDrop, t));
      shot2.style.opacity = String(1 - seg(p, T.drag2[0] + (T.drag2[1] - T.drag2[0]) * 0.7, T.drag2[1]));
    }

    /* folder */
    const fr = folderRect();
    folder.style.left = fr.x + "px";
    folder.style.top = fr.y + "px";
    folder.style.width = fr.w + "px";
    const folderT = easeBack(seg(p, T.folderPop[0], T.folderPop[1]));
    const folderO = p >= T.folderPop[0] ? 1 : 0;
    folder.style.opacity = String(folderO * clamp01(folderT) * lerp(1, 0.68, overviewT));
    folder.style.transform = `scale(${lerp(0.5, 1, clamp01(folderT))})`;
    const dragging =
      (p >= T.drag1[0] + 0.006 && p <= T.drag1[1]) ||
      (p >= T.drag2[0] + 0.006 && p <= T.drag2[1]);
    folder.classList.toggle("glow", dragging);
    moveLabel.style.opacity = dragging ? "1" : "0";
    moveLabel.style.left = fr.x + fr.w + 10 + "px";
    moveLabel.style.top = fr.y + fr.w * 0.35 + "px";

    /* cursor */
    const cp = cursorPos(p);
    cursor.style.left = cp.x - 3 + "px";
    cursor.style.top = cp.y - 2 + "px";
    const cursorO =
      Math.min(seg(p, 0.108, 0.122), 1 - seg(p, 0.79, 0.81));
    cursor.style.opacity = String(clamp01(cursorO));
    const grabbing = dragging || (p >= T.crop[0] && p <= T.crop[1]);
    cursor.style.transform = grabbing ? "scale(0.86)" : "scale(1)";

    updateRing(p);
  }

  /* ---------------- reduced motion: static annotated hero ---------------- */

  function applyStaticState() {
    document.body.classList.add("reduced");
    chrome.style.opacity = "0";
    backdrop.style.opacity = "0";
    const np = notePos();
    note.style.left = np.x + "px";
    note.style.top = np.y + "px";
    note.style.opacity = "0.9";
    typed.textContent = NOTE_TEXT;
    penPath.style.strokeDasharray = "none";
    penPath.style.strokeDashoffset = "0";
    penPath.style.opacity = "0.9";
    const fr = folderRect();
    folder.style.left = fr.x + "px";
    folder.style.top = fr.y + "px";
    folder.style.width = fr.w + "px";
    folder.style.opacity = "0.9";
    const s2rest = shot2Rest();
    applyRect(shot2, s2rest);
    shot2.style.opacity = "0.9";
  }

  /* ---------------- wiring ---------------- */

  function progress() {
    const r = scrolly.getBoundingClientRect();
    const total = r.height - window.innerHeight;
    return total > 0 ? clamp01(-r.top / total) : 0;
  }

  penLen = penPath.getTotalLength();

  const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  if (reduced) {
    applyStaticState();
    return;
  }

  let ticking = false;
  function onScroll() {
    if (ticking) return;
    ticking = true;
    requestAnimationFrame(() => {
      update(progress());
      ticking = false;
    });
  }

  window.addEventListener("scroll", onScroll, { passive: true });
  window.addEventListener("resize", onScroll);
  update(progress());
})();
