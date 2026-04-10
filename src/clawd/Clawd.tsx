import { useState } from "react";
import "./clawd.css";

// Two versions: narrow and wide, extracted from Claude Code CLI source
const narrowArt = [
  { parts: [{ text: "            ░░░░░░                                        ", cls: "" }] },
  { parts: [{ text: "    ░░░   ░░░░░░░░░░                                      ", cls: "" }] },
  { parts: [{ text: "   ░░░░░░░░░░░░░░░░░░░                                    ", cls: "" }] },
  { parts: [{ text: "                                                          ", cls: "" }] },
  { parts: [{ text: "                           ░░░░", cls: "dim" }, { text: "                     ██    ", cls: "" }] },
  { parts: [{ text: "                         ░░░░░░░░░░", cls: "dim" }, { text: "               ██▒▒██  ", cls: "" }] },
  { parts: [{ text: "                                            ▒▒      ██   ▒", cls: "" }] },
  { parts: [{ text: "      ", cls: "" }, { text: " █████████ ", cls: "body" }, { text: "                         ▒▒░░▒▒      ▒ ▒▒", cls: "" }] },
  { parts: [{ text: "      ", cls: "" }, { text: "██▄█████▄██", cls: "body-bg" }, { text: "                           ▒▒         ▒▒ ", cls: "" }] },
  { parts: [{ text: "      ", cls: "" }, { text: " █████████ ", cls: "body" }, { text: "                          ░          ▒   ", cls: "" }] },
  { parts: [{ text: "…………………", cls: "" }, { text: "█ █   █ █", cls: "body" }, { text: "……………………………………………………………………░…………………………▒…………", cls: "" }] },
];

const wideArt = [
  { parts: [{ text: "                                                          ", cls: "" }] },
  { parts: [{ text: "     *                                       █████▓▓░     ", cls: "" }] },
  { parts: [{ text: "                                 *         ███▓░     ░░   ", cls: "" }] },
  { parts: [{ text: "            ░░░░░░                        ███▓░           ", cls: "" }] },
  { parts: [{ text: "    ░░░   ░░░░░░░░░░                      ███▓░           ", cls: "" }] },
  { parts: [
    { text: "   ░░░░░░░░░░░░░░░░░░░    ", cls: "" },
    { text: "*", cls: "bold" },
    { text: "                ██▓░░      ▓   ", cls: "" },
  ] },
  { parts: [{ text: "                                             ░▓▓███▓▓░    ", cls: "" }] },
  { parts: [{ text: " *                                 ░░░░                   ", cls: "dim" }] },
  { parts: [{ text: "                                 ░░░░░░░░                 ", cls: "dim" }] },
  { parts: [{ text: "                               ░░░░░░░░░░░░░░░░           ", cls: "dim" }] },
  { parts: [{ text: "      ", cls: "" }, { text: " █████████ ", cls: "body" }, { text: "                                       ", cls: "" }, { text: "*", cls: "dim" }, { text: " ", cls: "" }] },
  { parts: [{ text: "      ", cls: "" }, { text: "██▄█████▄██", cls: "body-bg" }, { text: "                        ", cls: "" }, { text: "*", cls: "bold" }, { text: "                ", cls: "" }] },
  { parts: [{ text: "       ", cls: "" }, { text: " █████████ ", cls: "body" }, { text: "                                          ", cls: "" }] },
];

type Variant = "narrow" | "wide" | "auto";

export function Clawd({ variant = "auto" }: { variant?: Variant }) {
  const [v, setV] = useState<"narrow" | "wide">(variant === "auto" ? "wide" : variant);

  if (variant === "auto") {
    const mq = window.matchMedia("(min-width: 640px)");
    const handler = () => setV(mq.matches ? "wide" : "narrow");
    mq.addEventListener("change", handler);
    // Initial
    if (mq.matches !== (v === "wide")) setV(mq.matches ? "wide" : "narrow");
  }

  const art = v === "wide" ? wideArt : narrowArt;

  return (
    <pre className="clawd">
      {art.map((line, i) => (
        <div key={i} className="clawd-line">
          {line.parts.map((part, j) => (
            <span key={j} className={part.cls ? `clawd-${part.cls}` : undefined}>
              {part.text}
            </span>
          ))}
        </div>
      ))}
    </pre>
  );
}
