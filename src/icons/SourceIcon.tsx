import type { SessionStatus } from "../types";
import "./source-icon.css";

const SOURCE_COLORS: Record<string, string> = {
  "claude-code": "#D97757",
  cc: "#D97757",
  cursor: "#00D4AA",
  codex: "#10A37F",
  build: "#4CAF50",
  make: "#4CAF50",
  deploy: "#4FC3F7",
  test: "#9C6ADE",
  cli: "#43A047",
};

const DEFAULT_COLOR = "#88AAFF";

/** Hash a string to a vibrant HSL color (fixed saturation/lightness for dark bg) */
function hashColor(seed: string): string {
  let hash = 0;
  for (let i = 0; i < seed.length; i++) {
    hash = seed.charCodeAt(i) + ((hash << 5) - hash);
  }
  const hue = ((hash % 360) + 360) % 360;
  return `hsl(${hue}, 65%, 60%)`;
}

/** Darken a color for shadow edges */
function darkenColor(color: string): string {
  if (color.startsWith("hsl(")) {
    return color.replace(/,\s*([\d.]+)%\)$/, (_, l) => `, ${Math.max(0, Number(l) - 18)}%)`);
  }
  if (/^#[0-9a-f]{6}$/i.test(color)) {
    const r = parseInt(color.slice(1, 3), 16);
    const g = parseInt(color.slice(3, 5), 16);
    const b = parseInt(color.slice(5, 7), 16);
    const f = 0.72;
    return `rgb(${Math.round(r * f)},${Math.round(g * f)},${Math.round(b * f)})`;
  }
  return color;
}

/** Blush tint: 40% opacity version of the body color */
function blushColor(color: string): string {
  if (color.startsWith("hsl(")) {
    return color.replace("hsl(", "hsla(").replace(")", ", 0.4)");
  }
  return `${color}66`;
}

// C1-a shape: 18×18 teardrop octopus (水滴弹 body, 4 tentacles, no highlights/ornaments)
// Each entry: [startX, endX] inclusive, for each row y=0..17
const SHAPE_ROWS: [number, number][][] = [
  [[8, 9]],                                          // 0
  [[7, 10]],                                         // 1
  [[6, 11]],                                         // 2
  [[5, 12]],                                         // 3
  [[4, 13]],                                         // 4
  [[4, 13]],                                         // 5
  [[3, 14]],                                         // 6
  [[2, 15]],                                         // 7
  [[2, 15]],                                         // 8
  [[1, 16]],                                         // 9
  [[1, 16]],                                         // 10
  [[2, 15]],                                         // 11
  [[3, 14]],                                         // 12
  [[4, 13]],                                         // 13
  [[4, 13]],                                         // 14
  [[4, 5], [7, 8], [10, 11], [13, 14]],             // 15 – tentacles top
  [[3, 4], [7, 8], [10, 11], [14, 15]],             // 16 – tentacles mid
  [[4, 4], [8, 8], [11, 11], [14, 14]],             // 17 – tentacle tips
];

// Precompute body cell set and split into normal vs shadow cells.
// Shadow: right edge (!hasRight) | bottom edge (!hasBottom) | tentacle left edge (y≥15 && !hasLeft)
function buildShapeCells() {
  const body = new Set<string>();
  SHAPE_ROWS.forEach((segs, y) => {
    segs.forEach(([s, e]) => {
      for (let x = s; x <= e; x++) body.add(`${x},${y}`);
    });
  });

  const normal: [number, number][] = [];
  const shadow: [number, number][] = [];
  body.forEach((k) => {
    const [x, y] = k.split(",").map(Number);
    const isShadow =
      !body.has(`${x + 1},${y}`) ||
      !body.has(`${x},${y + 1}`) ||
      (y >= 15 && !body.has(`${x - 1},${y}`));
    (isShadow ? shadow : normal).push([x, y]);
  });

  return { normal, shadow };
}

const { normal: BODY_NORMAL, shadow: BODY_SHADOW } = buildShapeCells();

// Cheeks: two pixels each side at row 10 (widest area)
const CHEEK_CELLS: [number, number][] = [[4, 10], [5, 10], [13, 10], [14, 10]];

type Pixel = [number, number, string];

// Eyes sit at left=[5,7], right=[11,7] (top-left corner of 2×2 eye area)
// Mouth center at cx=8, cy=11
function getExpression(status: SessionStatus): Pixel[] {
  const W = "#FFFFFF";
  const D = "#1a1a2e";

  switch (status) {
    case "pending":
      return [
        // Open eyes: 2×2 white, pupils facing inward
        [5, 7, W], [6, 7, W], [5, 8, W], [6, 8, D],    // left eye, pupil right
        [11, 7, W], [12, 7, W], [11, 8, D], [12, 8, W], // right eye, pupil left
        // Smile: corners up, center dips
        [6, 11, D], [7, 12, D], [8, 12, D], [9, 12, D], [10, 11, D],
      ];
    case "running":
      return [
        // Half-lid squint: ink on lower row only (top row stays body color)
        [5, 8, D], [6, 8, D],
        [11, 8, D], [12, 8, D],
        // Smile
        [6, 11, D], [7, 12, D], [8, 12, D], [9, 12, D], [10, 11, D],
      ];
    case "success":
      return [
        // ^^ happy eyes: ink on top row only
        [5, 7, D], [6, 7, D],
        [11, 7, D], [12, 7, D],
        // Smile
        [6, 11, D], [7, 12, D], [8, 12, D], [9, 12, D], [10, 11, D],
      ];
    case "failure":
      return [
        // >< 闭眼：每只眼取 2×2 的对角两个像素，像斜线
        [5, 7, D], [6, 8, D],
        [12, 7, D], [11, 8, D],
        // 嘴角下垂：中间高两边低，倒 V 形
        [6, 12, D], [7, 11, D], [8, 11, D], [9, 11, D], [10, 12, D],
      ];
  }
}

function PixelMonster({ color, status }: { color: string; status: SessionStatus }) {
  const shadowCol = darkenColor(color);
  const blush = blushColor(color);
  const expression = getExpression(status);

  return (
    <svg viewBox="0 0 18 18" className="source-svg" shapeRendering="crispEdges">
      {/* Body fill */}
      {BODY_NORMAL.map(([x, y]) => (
        <rect key={`b${x},${y}`} x={x} y={y} width={1} height={1} fill={color} />
      ))}
      {/* Shadow edges */}
      {BODY_SHADOW.map(([x, y]) => (
        <rect key={`s${x},${y}`} x={x} y={y} width={1} height={1} fill={shadowCol} />
      ))}
      {/* Cheek blush */}
      {CHEEK_CELLS.map(([x, y]) => (
        <rect key={`c${x},${y}`} x={x} y={y} width={1} height={1} fill={blush} />
      ))}
      {/* Expression (eyes + mouth) */}
      {expression.map(([x, y, fill], i) => (
        <rect key={`e${i}`} x={x} y={y} width={1} height={1} fill={fill} />
      ))}
    </svg>
  );
}

interface SourceIconProps {
  source: string | null;
  status?: SessionStatus;
  animate?: boolean;
  /** Unique seed (e.g. task_id) to generate a distinct color per session */
  colorSeed?: string;
}

export default function SourceIcon({ source, status = "pending", animate = true, colorSeed }: SourceIconProps) {
  const key = source?.toLowerCase().replace(/[\s_]/g, "-") ?? "";
  const color = colorSeed ? hashColor(colorSeed) : (SOURCE_COLORS[key] || DEFAULT_COLOR);
  const animClass = animate ? status : "";

  return (
    <div className={`source-icon ${animClass}`}>
      <PixelMonster color={color} status={status} />
    </div>
  );
}
