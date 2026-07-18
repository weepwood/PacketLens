interface TrafficPoint {
  timestamp: number;
  bytesPerSecond: number;
  packetsPerSecond: number;
}

interface TrafficChartProps {
  points: TrafficPoint[];
}

function buildPath(values: number[], width: number, height: number): string {
  if (values.length === 0) return "";
  const max = Math.max(...values, 1);
  return values
    .map((value, index) => {
      const x = values.length === 1 ? width : (index / (values.length - 1)) * width;
      const y = height - (value / max) * (height - 12) - 6;
      return `${index === 0 ? "M" : "L"}${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join(" ");
}

export function TrafficChart({ points }: TrafficChartProps) {
  const values = points.map((point) => point.bytesPerSecond);
  const path = buildPath(values, 800, 210);

  return (
    <div className="traffic-chart" aria-label="最近一分钟实时网络吞吐量">
      <div className="chart-grid" />
      <svg viewBox="0 0 800 210" preserveAspectRatio="none" role="img">
        <defs>
          <linearGradient id="trafficFill" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor="rgba(55, 214, 181, 0.42)" />
            <stop offset="100%" stopColor="rgba(55, 214, 181, 0)" />
          </linearGradient>
        </defs>
        {path ? (
          <>
            <path d={`${path} L800,210 L0,210 Z`} fill="url(#trafficFill)" />
            <path d={path} fill="none" stroke="#37d6b5" strokeWidth="2.4" vectorEffect="non-scaling-stroke" />
          </>
        ) : null}
      </svg>
      {points.length === 0 ? <span className="chart-empty">启动捕获后显示实时曲线</span> : null}
    </div>
  );
}
