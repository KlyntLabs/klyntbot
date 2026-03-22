import { cn } from "@shared/lib/cn";

export interface ThinkingDotsProps {
  /** Dot radius in SVG units. sm=2, md=3, lg=4 */
  size?: "sm" | "md" | "lg";
  className?: string;
}

const config = {
  sm: { r: 2, w: 20, h: 6, cx: [3, 10, 17] },
  md: { r: 3, w: 28, h: 8, cx: [4, 14, 24] },
  lg: { r: 4, w: 36, h: 10, cx: [5, 18, 31] },
};

export function ThinkingDots({ size = "md", className }: ThinkingDotsProps) {
  const { r, w, h, cx } = config[size];
  return (
    <svg
      width={w}
      height={h}
      viewBox={`0 0 ${w} ${h}`}
      className={cn("text-brand", className)}
      aria-label="Loading"
    >
      {cx.map((x, i) => (
        <circle key={x} cx={x} cy={h / 2} r={r} fill="currentColor" opacity="0.3">
          <animate
            attributeName="opacity"
            values="0.3;1;0.3"
            dur="1s"
            repeatCount="indefinite"
            begin={`${i * 0.2}s`}
          />
        </circle>
      ))}
    </svg>
  );
}
