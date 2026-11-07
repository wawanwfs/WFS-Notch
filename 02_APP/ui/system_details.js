'use strict';
// Optional native read-only telemetry. The browser preview never fabricates Windows readings.
let systemDetailsInitialized = false;
let systemDetailsPending = false;
let systemDetailsLastFetch = 0;
function systemBytes(n) {
  if (!Number.isFinite(n) || n < 0) return '—';
  if (n < 1024) return Math.floor(n) + ' B';
  const units = ['KiB', 'MiB', 'GiB', 'TiB', 'PiB'];
  const exp = Math.min(4, Math.floor(Math.log(n) / Math.log(1024)) - 1);
  return (n / (1024 ** (exp + 1))).toFixed(1) + ' ' + units[exp];
}
function systemDetailsLine(title, line, percentage) {
  const wrapper = document.createElement('div');
  wrapper.className = 'detail-row';
  const label = document.createElement('div');
  label.className = 'detail-label';
  const head = document.createElement('b');
  head.textContent = title;
  const sub = document.createElement('small');
  sub.textContent = line;
  label.append(head, sub);
  wrapper.append(label);
  if (Number.isFinite(percentage)) {
    const meter = document.createElement('div');
    meter.className = 'detail-meter';
    const fill = document.createElement('span');
    fill.style.width = Math.max(0, Math.min(100, percentage)).toFixed(1) + '%';
    meter.append(fill);
    wrapper.append(meter);
  }
  return wrapper;
}
function renderSystemDetails(snapshot) {
  const disks = document.querySelector('#system-disks');
  const networks = document.querySelector('#system-networks');
  disks.replaceChildren();
  networks.replaceChildren();
  if (!snapshot) {
    disks.textContent = 'Native Windows readings unavailable in browser preview.';
    networks.textContent = 'Native Windows readings unavailable in browser preview.';
    return;
  }
  if (!snapshot.disks?.length) disks.textContent = 'No disk information available from Windows.';
  for (const disk of (snapshot.disks || []).slice(0, 12)) {
    const usage = disk.total_bytes > 0 ? 100 * disk.used_bytes / disk.total_bytes : NaN;
    disks.append(systemDetailsLine(
      disk.label || disk.mount || 'Volume',
      (disk.mount || '') + ' · ' + systemBytes(disk.used_bytes) + ' of ' + systemBytes(disk.total_bytes),
      usage
    ));
  }
  if ((snapshot.disks || []).length > 12) {
    disks.append(systemDetailsLine('Additional volumes omitted', 'Displaying first 12 volumes'));
  }
  if (!snapshot.networks?.length) networks.textContent = 'No network counters available from Windows.';
  for (const iface of (snapshot.networks || []).slice(0, 12)) {
    networks.append(systemDetailsLine(
      iface.name || 'Network adapter',
      'Received ' + systemBytes(iface.received_bytes) + ' · Sent ' + systemBytes(iface.transmitted_bytes)
    ));
  }
  if ((snapshot.networks || []).length > 12) {
    networks.append(systemDetailsLine('Additional adapters omitted', 'Displaying first 12 adapters'));
  }
}
async function refreshSystemDetails(force = false) {
  if (!nativeInvoke || systemDetailsPending) return;
  const panel = document.querySelector('#panel-system');
  if (!panel?.classList.contains('active')) return;
  if (!force && Date.now() - systemDetailsLastFetch < 15000) return;
  systemDetailsPending = true;
  try {
    renderSystemDetails(await invoke('get_system_details'));
    systemDetailsLastFetch = Date.now();
  } catch (error) {
    document.querySelector('#system-disks').textContent = 'Disk snapshot unavailable.';
    document.querySelector('#system-networks').textContent = 'Network snapshot unavailable.';
    showError(error);
  } finally {
    systemDetailsPending = false;
  }
}
function initSystemDetails() {
  if (systemDetailsInitialized) return;
  systemDetailsInitialized = true;
  renderSystemDetails(null);
  document.querySelector('#system-refresh').addEventListener('click', () => {
    void refreshSystemDetails(true);
  });
  document.querySelector('[data-tab="system"]').addEventListener('click', () => {
    setTimeout(() => { void refreshSystemDetails(); }, 0);
  });
  setInterval(() => { void refreshSystemDetails(); }, 5000);
}
if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', initSystemDetails, {once: true});
else initSystemDetails();
