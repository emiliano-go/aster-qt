#!/usr/bin/env node
// Playwright check for theme token contrast.
// Playwright cannot drive a Qt Quick window, so this verifies the CSS theme
// tokens that QML consumes (including user CSS themes) and the QML usage.
const fs = require('fs');
const path = require('path');
const os = require('os');
const { chromium } = require('playwright');

const root = path.resolve(__dirname, '..');
const template = fs.readFileSync(path.join(root, 'themes/template.css'), 'utf8');
const userDir = path.join(os.homedir(), '.local/share/com.astermail.qt/themes');
const themes = [{ name: 'template', css: template }];
if (fs.existsSync(userDir)) {
  for (const file of fs.readdirSync(userDir).filter((f) => f.endsWith('.css'))) {
    const css = fs.readFileSync(path.join(userDir, file), 'utf8');
    themes.push({ name: file, css: `${template}\n${css}` });
  }
}

const qml = fs.readFileSync(path.join(root, 'crates/aster-qt/qml/Main.qml'), 'utf8');
if (!qml.includes('message-divider')) {
  console.error('FAIL: Main.qml does not use the message-divider theme token');
  process.exit(1);
}

function luminance([r, g, b]) {
  const channel = (v) => {
    v /= 255;
    return v <= 0.04045 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4);
  };
  return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
}
function parseRgb(value) {
  const match = value.match(/\d+/g);
  if (!match) throw new Error(`unparsable color: ${value}`);
  return match.slice(0, 3).map(Number);
}
function contrast(a, b) {
  const la = luminance(parseRgb(a));
  const lb = luminance(parseRgb(b));
  const hi = Math.max(la, lb);
  const lo = Math.min(la, lb);
  return (hi + 0.05) / (lo + 0.05);
}

(async () => {
  const browser = await chromium.launch({
    headless: true,
    executablePath: fs.existsSync('/usr/bin/chromium') ? '/usr/bin/chromium' : undefined,
  });
  const page = await browser.newPage();
  let failed = false;
  for (const theme of themes) {
    await page.setContent(`<style>${theme.css}</style>
      <div id="bg" style="background:var(--bg)"></div>
      <div id="divider" style="background:var(--message-divider)"></div>
      <div id="text" style="color:var(--text)"></div>
      <div id="muted" style="color:var(--muted)"></div>`);
    const colors = await page.evaluate(() => {
      const read = (id, prop) => getComputedStyle(document.getElementById(id))[prop];
      return {
        bg: read('bg', 'backgroundColor'),
        divider: read('divider', 'backgroundColor'),
        text: read('text', 'color'),
        muted: read('muted', 'color'),
      };
    });
    const dividerContrast = contrast(colors.divider, colors.bg);
    const textContrast = contrast(colors.text, colors.bg);
    const mutedContrast = contrast(colors.muted, colors.bg);
    const ok = dividerContrast >= 1.25 && textContrast >= 4.5 && mutedContrast >= 2.5;
    if (!ok) failed = true;
    console.log(
      `${ok ? 'PASS' : 'FAIL'} ${theme.name}: divider/bg ${dividerContrast.toFixed(2)} ` +
      `(need >=1.25), text/bg ${textContrast.toFixed(2)} (need >=4.5), muted/bg ${mutedContrast.toFixed(2)} (need >=2.5)`
    );
  }
  await browser.close();
  process.exit(failed ? 1 : 0);
})().catch((err) => {
  console.error(err);
  process.exit(1);
});
