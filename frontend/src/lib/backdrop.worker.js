const j = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5].map(
  (o) => (o + 0.5) / 16,
);

let f;
let m = null;
let p = [0, 0, 0];
let b = [0, 0, 0];
let x = false;

const z = (o) => o - Math.floor(o);
const M = (o, t) => z(Math.sin(o * 127.1 + t * 311.7) * 43758.5453);

function A(o, t) {
  const a = Math.floor(o);
  const n = Math.floor(t);
  const e = o - a;
  const s = t - n;
  const i = e * e * (3 - 2 * e);
  const h = s * s * (3 - 2 * s);
  const c = M(a, n);
  const l = M(a + 1, n);
  const E = M(a, n + 1);
  const r = M(a + 1, n + 1);
  return c + (l - c) * i + (E - c) * h + (c - l - E + r) * i * h;
}

function P(o, t) {
  let a = 0;
  let n = 0.5;
  for (let e = 0; e < 4; e++) {
    a += n * A(o, t);
    [o, t] = [1.6 * o - 1.2 * t, 1.2 * o + 1.6 * t];
    n *= 0.5;
  }
  return a / 0.9375;
}

function D(o, t, a) {
  const n = o * 3 * 0.0022;
  const e = t * 3 * 0.0022;
  const s = P(n, e + a * 0.025);
  const i = P(n + 5.2 - a * 0.02, e + 1.3);
  return P(n + 1.7 * s + a * 0.01, e + 1.7 * i - a * 0.008);
}

function k(o) {
  if (!f || !m) return;
  const t = f.width;
  const a = f.height;
  const n = Math.ceil(t / 4) + 1;
  const e = Math.ceil(a / 4) + 1;
  const s = new Float32Array(n * e);
  for (let c = 0; c < e; c++) {
    for (let l = 0; l < n; l++) {
      s[c * n + l] = D(l * 4, c * 4, o);
    }
  }
  const i = m.createImageData(t, a);
  const h = i.data;
  for (let c = 0; c < a; c++) {
    const l = Math.floor(c / 4);
    const E = (c % 4) / 4;
    for (let r = 0; r < t; r++) {
      const I = Math.floor(r / 4);
      const C = (r % 4) / 4;
      const u = l * n + I;
      const y = s[u] + (s[u + 1] - s[u]) * C;
      const B = s[u + n] + (s[u + n + 1] - s[u + n]) * C;
      const d = y + (B - y) * E;
      const F = j[(c % 4) * 4 + (r % 4)];
      const R =
        Math.min(
          Math.max(
            Math.max(1 - Math.abs(d - 0.47) * 22, 1 - Math.abs(d - 0.66) * 30),
            0,
          ),
          1,
        ) * 0.42;
      const T = Math.min(Math.max((d - 0.52) / 0.26, 0), 1);
      const Y = T * T * (3 - 2 * T) * 0.17;
      const w = R > F ? b : Y > F ? p : null;
      if (w) {
        const g = (c * t + r) * 4;
        h[g] = w[0];
        h[g + 1] = w[1];
        h[g + 2] = w[2];
        h[g + 3] = 255;
      }
    }
  }
  m.putImageData(i, 0, 0);
}

let S = Math.random() * 1000;
let L = performance.now();

function v() {
  const o = performance.now();
  S += Math.min(o - L, 200) / 1e3;
  L = o;
  k(S);
  if (!x) setTimeout(v, 1e3 / 15);
}

onmessage = (o) => {
  const t = o.data;
  if (t.canvas) {
    f = t.canvas;
    m = f.getContext("2d");
    x = t.still;
  }
  if (t.width) {
    f.width = t.width;
    f.height = t.height;
  }
  if (t.fg) {
    const a = (n, e, s) => n.map((i, h) => Math.round(i + (e[h] - i) * s));
    p = a(t.bg, t.fg, 0.22);
    b = a(t.bg, t.fg, 0.45);
  }
  if (t.canvas) v();
  else k(S);
};
