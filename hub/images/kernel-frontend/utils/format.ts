//  時刻と時間の見せ方をここに集める。
//  表示はすべて日本標準時。桁を揃えて読めるようにする。

const JST = "Asia/Tokyo";

export function parseUtc(value: unknown): Date | null {
  if (!value) return null;
  const date = new Date(String(value));
  return Number.isNaN(date.getTime()) ? null : date;
}

/*  月日と時刻。一覧で桁が揃う形。 */
export function fmtJst(date: Date | null): string {
  if (!date) return "-";
  const parts = new Intl.DateTimeFormat("ja-JP", {
    timeZone: JST,
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  }).formatToParts(date);
  const get = (type: string) => parts.find((p) => p.type === type)?.value || "";
  return `${get("month")}-${get("day")} ${get("hour")}:${get("minute")}:${get("second")}`;
}

/*  時刻だけ。時間軸の目盛りに使う。 */
export function fmtClock(date: Date | null): string {
  if (!date) return "-";
  return new Intl.DateTimeFormat("ja-JP", {
    timeZone: JST,
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(date);
}

/*  かかった時間。終わりが無ければ今までの経過。 */
export function fmtDuration(start: Date | null, end: Date | null): string {
  if (!start) return "-";
  const stop = end || new Date();
  let secs = Math.floor((stop.getTime() - start.getTime()) / 1000);
  if (secs < 0) secs = 0;
  if (secs < 60) return `${secs} 秒`;
  if (secs < 3600) return `${Math.floor(secs / 60)} 分 ${secs % 60} 秒`;
  return `${Math.floor(secs / 3600)} 時間 ${Math.floor((secs % 3600) / 60)} 分`;
}

/*  何分前かを言葉で。 */
export function fmtAgo(date: Date | null): string {
  if (!date) return "-";
  const secs = Math.floor((Date.now() - date.getTime()) / 1000);
  if (secs < 60) return "たったいま";
  if (secs < 3600) return `${Math.floor(secs / 60)} 分前`;
  if (secs < 86400) return `${Math.floor(secs / 3600)} 時間前`;
  return `${Math.floor(secs / 86400)} 日前`;
}

/*  見る時間幅の言い方。 */
export function windowLabel(minutes: number): string {
  if (minutes < 60) return `直近 ${minutes} 分`;
  if (minutes % 1440 === 0) return `直近 ${minutes / 1440} 日`;
  return `直近 ${minutes / 60} 時間`;
}

/*  バイト数。二進の接頭辞を使う。 */
export function fmtBytes(bytes: number | null | undefined): string {
  if (bytes === null || bytes === undefined) return "-";
  const units = ["バイト", "KiB", "MiB", "GiB", "TiB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return unit === 0 ? `${value} ${units[0]}` : `${value.toFixed(1)} ${units[unit]}`;
}
