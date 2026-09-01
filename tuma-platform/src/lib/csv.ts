/**
 * Client-side CSV export for table views (toolbar "Export CSV"). Only
 * ever exports data the server already returned — no invention.
 */
export function toCsv(
  columns: Array<{ header: string; value: (row: never) => string }>,
  rows: never[],
): string {
  const escape = (v: string) =>
    /[",\n]/.test(v) ? `"${v.replaceAll('"', '""')}"` : v
  const head = columns.map((c) => escape(c.header)).join(',')
  const body = rows
    .map((row) => columns.map((c) => escape(c.value(row))).join(','))
    .join('\n')
  return `${head}\n${body}`
}

/** Trigger a browser download of CSV text. */
export function downloadCsv(filename: string, csv: string) {
  const blob = new Blob([csv], { type: 'text/csv;charset=utf-8' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = filename
  a.click()
  URL.revokeObjectURL(url)
}
