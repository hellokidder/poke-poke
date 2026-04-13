import type { TaskStatus } from "../types";
import "./source-icon.css";

const SOURCE_COLORS: Record<string, string> = {
  "claude-code": "#D97757",
  cc: "#D97757",
  cursor: "#00D4AA",
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

// 16x12 pixel grid: 1 = body color, 0 = empty (filled by expression overlay)
const BODY: number[][] = [
  [0,0,0,0,1,1,1,1,1,1,1,1,0,0,0,0], // row 0:  dome top (smooth, no horns)
  [0,0,1,1,1,1,1,1,1,1,1,1,1,1,0,0], // row 1:  head top
  [0,1,1,1,1,1,1,1,1,1,1,1,1,1,1,0], // row 2:  head
  [1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1], // row 3:  face
  [1,1,1,0,0,1,1,1,1,1,1,0,0,1,1,1], // row 4:  eye sockets top
  [1,1,1,0,0,1,1,1,1,1,1,0,0,1,1,1], // row 5:  eye sockets bottom
  [1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1], // row 6:  cheeks
  [1,1,1,1,1,1,0,0,0,0,1,1,1,1,1,1], // row 7:  mouth top
  [1,1,1,1,1,1,0,0,0,0,1,1,1,1,1,1], // row 8:  mouth bottom
  [0,1,1,1,1,1,1,1,1,1,1,1,1,1,1,0], // row 9:  body taper
  [0,1,1,0,1,1,0,1,1,0,1,1,0,1,1,0], // row 10: 5 tentacle tops (symmetric)
  [0,0,1,0,0,1,0,0,1,0,0,1,0,0,1,0], // row 11: 5 tentacle tips
];

type Pixel = [number, number, string];

function getExpression(status: TaskStatus, bodyColor: string): Pixel[] {
  const W = "#FFFFFF";
  const D = "#1a1a2e";
  const C = bodyColor;

  switch (status) {
    case "pending":
      // Calm: white eyes with pupils looking center, flat mouth
      return [
        [3,4,W], [4,4,W], [3,5,W], [4,5,D],    // left eye, pupil bottom-right
        [11,4,W],[12,4,W],[11,5,D],[12,5,W],    // right eye, pupil bottom-left
        [6,7,C], [7,7,D], [8,7,D], [9,7,C],    // mouth: flat line
        [6,8,C], [7,8,C], [8,8,C], [9,8,C],    // mouth bottom: body
      ];
    case "running":
      // Focused: squint eyes (body top, dark bottom), flat mouth
      return [
        [3,4,C], [4,4,C], [3,5,D], [4,5,D],    // left eye squint
        [11,4,C],[12,4,C],[11,5,D],[12,5,D],    // right eye squint
        [6,7,C], [7,7,D], [8,7,D], [9,7,C],    // mouth: flat line
        [6,8,C], [7,8,C], [8,8,C], [9,8,C],    // mouth bottom: body
      ];
    case "success":
      // Happy: ^^ closed eyes, big smile arc
      return [
        [3,4,D], [4,4,D], [3,5,C], [4,5,C],    // left eye happy ^^
        [11,4,D],[12,4,D],[11,5,C],[12,5,C],    // right eye happy ^^
        [6,7,D], [7,7,C], [8,7,C], [9,7,D],    // mouth top: smile ends
        [6,8,C], [7,8,D], [8,8,D], [9,8,C],    // mouth bottom: smile curve
      ];
    case "failed":
      // Dead: X eyes, frown
      return [
        [3,4,D], [4,4,C], [3,5,C], [4,5,D],    // left eye: \ diagonal
        [11,4,C],[12,4,D],[11,5,D],[12,5,C],    // right eye: / diagonal
        [6,7,C], [7,7,D], [8,7,D], [9,7,C],    // mouth top: frown ends
        [6,8,D], [7,8,C], [8,8,C], [9,8,D],    // mouth bottom: frown curve
      ];
  }
}

function PixelMonster({ color, status }: { color: string; status: TaskStatus }) {
  // Lighter shade for cheek blush
  const blush = `${color}66`;

  const expression = getExpression(status, color);

  return (
    <svg viewBox="0 0 16 12" className="source-svg" shapeRendering="crispEdges">
      {/* Body pixels */}
      {BODY.map((row, y) =>
        row.map((cell, x) =>
          cell ? <rect key={`${x}-${y}`} x={x} y={y} width={1} height={1} fill={color} /> : null
        )
      )}
      {/* Expression overlay (eyes + mouth) */}
      {expression.map(([x, y, fill], i) => (
        <rect key={`e${i}`} x={x} y={y} width={1} height={1} fill={fill as string} />
      ))}
      {/* Cheek blush */}
      <rect x={2} y={6} width={1} height={1} fill={blush} />
      <rect x={13} y={6} width={1} height={1} fill={blush} />
      {/* Shine highlight */}
      <rect x={4} y={2} width={1} height={1} fill="rgba(255,255,255,0.3)" />
      <rect x={5} y={2} width={1} height={1} fill="rgba(255,255,255,0.15)" />
    </svg>
  );
}

interface SourceIconProps {
  source: string | null;
  status?: TaskStatus;
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
