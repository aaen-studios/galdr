import { useEffect, useState } from "react";
import { useHistoryStore, formatBytes } from "../store/historyStore";
import type { UsageStats } from "../types";

export default function StatsPage() {
  const stats = useHistoryStore((s) => s.stats);
  const [data, setData] = useState<UsageStats | null>(null);

  useEffect(() => {
    stats().then(setData).catch(() => {});
  }, [stats]);

  if (!data) {
    return (
      <div className="page">
        <h2>ᛟ stats</h2>
        <div className="skeleton-bar" />
      </div>
    );
  }

  const maxRecent = Math.max(1, ...data.recent.map(([, c]) => c));

  return (
    <div className="page">
      <h2>ᛟ stats</h2>
      <p className="settings-hint" style={{ marginBottom: 16 }}>
        your casting history, aggregated. all data is local — nothing is sent anywhere.
      </p>

      <div className="stats-grid">
        <div className="stat-card">
          <div className="stat-card-label">total ops</div>
          <div className="stat-card-val">{data.totalOps}</div>
        </div>
        <div className="stat-card">
          <div className="stat-card-label">completed</div>
          <div className="stat-card-val" style={{ color: "var(--success)" }}>{data.completed}</div>
        </div>
        <div className="stat-card">
          <div className="stat-card-label">failed</div>
          <div className="stat-card-val" style={{ color: "var(--danger)" }}>{data.failed}</div>
        </div>
        <div className="stat-card">
          <div className="stat-card-label">output produced</div>
          <div className="stat-card-val">{formatBytes(data.totalOutputBytes)}</div>
        </div>
      </div>

      <div className="card">
        <label className="label">last 7 days</label>
        <div className="stat-spark">
          {data.recent.map(([date, count]) => {
            const h = (count / maxRecent) * 100;
            const day = new Date(date).toLocaleDateString(undefined, { weekday: "short" });
            return (
              <div
                key={date}
                className={`stat-spark-bar${count === maxRecent && count > 0 ? " peak" : ""}`}
                style={{ height: `${Math.max(h, 4)}%` }}
                title={`${day}: ${count}`}
              />
            );
          })}
        </div>
        <div className="row" style={{ justifyContent: "space-between", marginTop: 6 }}>
          {data.recent.map(([date]) => (
            <span key={date} style={{ fontSize: 9, color: "var(--fg-dim)" }}>
              {new Date(date).toLocaleDateString(undefined, { weekday: "narrow" })}
            </span>
          ))}
        </div>
      </div>

      {Object.keys(data.byOp).length > 0 && (
        <div className="card" style={{ marginTop: 16 }}>
          <label className="label">by operation</label>
          {Object.entries(data.byOp)
            .sort((a, b) => b[1] - a[1])
            .map(([op, count]) => {
              const pct = (count / data.totalOps) * 100;
              return (
                <div key={op} style={{ marginBottom: 8 }}>
                  <div className="row" style={{ justifyContent: "space-between", marginBottom: 2 }}>
                    <span style={{ fontSize: 12, color: "var(--fg)" }}>{op}</span>
                    <span style={{ fontSize: 11, color: "var(--fg-dim)" }}>{count}</span>
                  </div>
                  <div style={{ height: 3, background: "var(--fg-faint)" }}>
                    <div style={{ height: "100%", width: `${pct}%`, background: "var(--fg-dim)" }} />
                  </div>
                </div>
              );
            })}
        </div>
      )}
    </div>
  );
}
