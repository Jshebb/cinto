import { useId } from "react";
import type { SVGProps } from "react";

type AnimatedGridPatternProps = {
  size?: number;
  stroke?: string;
  strokeWidth?: number;
  duration?: number;
} & SVGProps<SVGSVGElement>;

export function AnimatedGridPattern({
  size = 64,
  stroke = "rgba(255, 255, 255, 0.12)",
  strokeWidth = 1,
  duration = 20,
  className,
  ...props
}: AnimatedGridPatternProps) {
  const patternId = useId();
  const gridSize = Math.max(8, size);

  return (
    <svg
      className={className}
      width="100%"
      height="100%"
      preserveAspectRatio="none"
      aria-hidden="true"
      {...props}
    >
      <defs>
        <pattern
          id={patternId}
          width={gridSize}
          height={gridSize}
          patternUnits="userSpaceOnUse"
        >
          <path
            d={`M ${gridSize} 0 L 0 0 0 ${gridSize}`}
            fill="none"
            stroke={stroke}
            strokeWidth={strokeWidth}
            vectorEffect="non-scaling-stroke"
            shapeRendering="crispEdges"
          />
          <animateTransform
            attributeName="patternTransform"
            type="translate"
            from="0 0"
            to={`${gridSize} ${gridSize}`}
            dur={`${duration}s`}
            repeatCount="indefinite"
          />
        </pattern>
      </defs>
      <rect width="100%" height="100%" fill={`url(#${patternId})`} />
    </svg>
  );
}
