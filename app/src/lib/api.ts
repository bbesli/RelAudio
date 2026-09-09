import { invoke } from "@tauri-apps/api/core";

export type DeviceKind = "output" | "input" | "monitor";

export interface Device {
  id: string;
  name: string;
  kind: DeviceKind;
  is_default: boolean;
  virtual_cable: boolean;
  /** Sanal kablolar arasında tercih sırası: 0 = kanonik stereo uç. */
  rank: number;
}

export interface Stats {
  server_running: boolean;
  server_target: string;
  server_packets: number;
  server_kbps: number;
  server_silent_ratio: number;
  server_send_errors: number;

  player_running: boolean;
  player_port: number;
  player_packets: number;
  player_kbps: number;
  player_lost: number;
  player_late: number;
  player_underruns: number;
  player_dropped: number;
  player_buffer_ms: number;
  player_peak: number;
  player_device_name: string;
  server_device_name: string;
  feedback_loop: boolean;
  /** Kulaklık modu bu makinede uzaktan başlatıldıysa: kim başlattı. */
  started_by: { name: string; address: string } | null;
  /** Ekranda gösterilecek eşleştirme kodu ve kalan saniye. */
  pairing_code: string | null;
  pairing_seconds: number;

  last_error: string | null;
}

export const listDevices = () => invoke<Device[]>("list_devices");

export const startServer = (target: string, device_id: string, source: "monitor" | "input") =>
  invoke<void>("start_server", { args: { target, device_id, source } });

export const stopServer = () => invoke<void>("stop_server");

export const startPlayer = (port: number, device_id: string, buffer_packets: number) =>
  invoke<void>("start_player", { args: { port, device_id, buffer_packets } });

export const stopPlayer = () => invoke<void>("stop_player");

export const getStats = () => invoke<Stats>("stats");

export const getLocalAddress = () => invoke<string | null>("local_address");

export interface Peer {
  id: string;
  name: string;
  address: string;
  port: number;
  os: string;
  listening: boolean;
  /** Eş tek düğmeyle uzaktan başlatılabiliyor mu? */
  can_remote_start: boolean;
}

export const getPeers = () => invoke<Peer[]>("peers");

export interface MicHint {
  paired_input: string | null;
  any_virtual: boolean;
}

export const getMicHint = (output_id: string) => invoke<MicHint>("mic_hint", { outputId: output_id });

export type HeadsetRole = "local" | "remote";

/** Çekirdeğin çözdüğü kulaklık aygıt planı — arayüz ve uzaktan gelen istek
 *  aynı politikayı kullansın diye Rust tarafında hesaplanıyor. */
export interface HeadsetPlan {
  capture: Device | null;
  play: Device | null;
  capture_kind: "input" | "monitor";
  /** "need_physical" | "need_cable" — çeviri anahtarına eşlenir. */
  problem: string | null;
  capture_choices: Device[];
  play_choices: Device[];
  paired_mic: string | null;
}

export const getHeadsetPlan = (role: HeadsetRole, savedPlay: string, savedCapture: string) =>
  invoke<HeadsetPlan>("headset_plan", { role, savedPlay, savedCapture });

export interface HeadsetStartResult {
  remote_started: boolean;
  remote_name: string;
  remote_paired_mic: string | null;
  /** Karşı taraf başlatılamadıysa sebebi; bu makine yine de çalışıyor. */
  remote_error: string | null;
  /** Sebebin çevrilebilir karşılığı ("disabled", "needs_pairing", ...). */
  remote_error_code: string | null;
  /** Karşı tarafla henüz eşleşmedik; kod ekranda gösteriliyor. */
  needs_pairing: boolean;
  /** Karşı makinede yazılacak 6 haneli kod. */
  pairing_code: string | null;
}

/** Karşı makinenin ekranındaki kodu girip eşleşir. Dönen değer karşı
 *  makinenin adı; hata `pair_wrong` gibi bir kod olarak fırlıyor. */
export const pairWithPeer = (peer_id: string, code: string) =>
  invoke<string>("pair_with_peer", { peerId: peer_id, code });

export const cancelPairing = () => invoke<void>("cancel_pairing");
export const isPaired = (peer_id: string) => invoke<boolean>("is_paired", { peerId: peer_id });
export const unpair = (peer_id: string) => invoke<void>("unpair", { peerId: peer_id });

export const startHeadset = (role: HeadsetRole, peer_id: string, address: string) =>
  invoke<HeadsetStartResult>("start_headset", { args: { role, peer_id, address } });

export const stopHeadset = () => invoke<void>("stop_headset");

export interface Config {
  language: string;
  minimize_to_tray: boolean;
  auto_listen: boolean;
  player_mode: string;
  player_port: number;
  player_device: string;
  player_buffer: number;
  server_source: string;
  server_device_monitor: string;
  server_device_input: string;
  server_target: string;
  headset_role: string;
  remote_control: boolean;
  paired: Record<string, { key: string; name: string }>;
}

export const getConfig = () => invoke<Config>("get_config");
export const setConfig = (config: Config) => invoke<void>("set_config", { config });
export const getConfigPath = () => invoke<string>("config_path");

export const setTrayLabels = (show: string, stop_all: string, quit: string) =>
  invoke<void>("set_tray_labels", { show, stopAll: stop_all, quit });
export const getLogPath = () => invoke<string>("log_file");
export const getDeviceName = () => invoke<string>("device_name");

export const setMinimizeToTray = (enabled: boolean) =>
  invoke<void>("set_minimize_to_tray", { enabled });
