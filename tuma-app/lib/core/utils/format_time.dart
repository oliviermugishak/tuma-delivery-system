// Shared time formatting for the app's clock-time labels.

/// The clock-time label — 24-hour HH:MM in the viewer's local time
/// ("14:05"). The shared home for every clock-time label: the detail
/// screen's delivered line and the status trail's milestone line both
/// speak this one spelling.
String clockTime(DateTime dt) {
  final local = dt.toLocal();
  final hour = local.hour.toString().padLeft(2, '0');
  final minute = local.minute.toString().padLeft(2, '0');
  return '$hour:$minute';
}
