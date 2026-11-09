// Renders scripts/demo/demo.html into docs/demo.gif.
// Run: npm run demo  (options: npm run demo -- --fps 15, or --still <seconds> for one PNG)
const { app, BrowserWindow } = require('electron');
const fs = require('node:fs');
const path = require('node:path');
const { GIFEncoder, quantize, applyPalette } = require('gifenc');

const args = process.argv.slice(2);
const option = (name, fallback) => {
  const i = args.indexOf(`--${name}`);
  return i === -1 ? fallback : args[i + 1];
};
const FPS = Number(option('fps', 15));
const STILL = option('still', null);
const ROOT = path.resolve(__dirname, '../..');
const OUT = path.join(ROOT, 'docs/demo.gif');
const WIDTH = 1100;
const HEIGHT = 680;

app.disableHardwareAcceleration();

async function frame(win, t) {
  await win.webContents.executeJavaScript(
    `render(${t}); new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)))`,
  );
  const image = await win.webContents.capturePage();
  return image.resize({ width: WIDTH, height: HEIGHT, quality: 'best' });
}

app.whenReady().then(async () => {
  const win = new BrowserWindow({
    width: WIDTH,
    height: HEIGHT,
    useContentSize: true,
    show: false,
    webPreferences: { offscreen: true },
  });
  await win.loadFile(path.join(__dirname, 'demo.html'));
  await fontsReady(win);

  if (STILL !== null) {
    const image = await frame(win, Number(STILL));
    const file = path.join(process.env.TMPDIR || '/tmp', `demo-${STILL}.png`);
    fs.writeFileSync(file, image.toPNG());
    console.log(file);
    app.quit();
    return;
  }

  const total = await win.webContents.executeJavaScript('window.TOTAL');
  const count = Math.ceil(total * FPS);
  const frames = [];
  for (let i = 0; i < count; i++) {
    const bgra = (await frame(win, i / FPS)).toBitmap();
    const rgba = Buffer.alloc(bgra.length);
    for (let p = 0; p < bgra.length; p += 4) {
      rgba[p] = bgra[p + 2];
      rgba[p + 1] = bgra[p + 1];
      rgba[p + 2] = bgra[p];
      rgba[p + 3] = 255;
    }
    const last = frames.at(-1);
    if (last && last.rgba.equals(rgba)) last.delay += 1000 / FPS;
    else frames.push({ rgba, delay: 1000 / FPS });
  }

  // One palette for every frame keeps static areas from flickering. Index 255 is
  // reserved for "unchanged since the previous frame", which compresses well.
  const sample = Buffer.concat(frames.filter((_, i) => i % 4 === 0).map((f) => f.rgba));
  const palette = quantize(sample, 255);
  while (palette.length < 255) palette.push([0, 0, 0]);
  palette.push([255, 0, 255]);
  const TRANSPARENT = 255;

  const gif = GIFEncoder();
  let previous = null;
  for (const [i, f] of frames.entries()) {
    const index = applyPalette(f.rgba, palette.slice(0, 255));
    const out = previous ? Uint8Array.from(index, (v, p) => (v === previous[p] ? TRANSPARENT : v)) : index;
    gif.writeFrame(out, WIDTH, HEIGHT, {
      palette: i === 0 ? palette : undefined,
      delay: Math.round(f.delay),
      transparent: i > 0,
      transparentIndex: TRANSPARENT,
      dispose: 1,
      repeat: 0,
    });
    previous = index;
  }
  gif.finish();
  fs.mkdirSync(path.dirname(OUT), { recursive: true });
  fs.writeFileSync(OUT, gif.bytes());
  console.log(`${OUT}: ${frames.length} frames, ${(fs.statSync(OUT).size / 1e6).toFixed(2)} MB, ${total.toFixed(1)}s`);
  app.quit();
});

function fontsReady(win) {
  return win.webContents.executeJavaScript('document.fonts.ready.then(() => true)');
}
