import { invoke } from "@tauri-apps/api/core";

export type DeviceKind = "output" | "input" | "monitor";

export interface Device {
  id: string;
  name: string;
  kind: DeviceKind;
  is_default: boolean;
}

export interface Stats {
  server_running: boolean;
  server_target: string;
  server_packets: number;
  server_kbps: number;
  server_silent_ratio: number;

  player_running: boolean;
  player_port: number;
  player_packets: number;
  player_kbps: number;
  player_lost: number;
  player_late: number;
  player_underruns: number;
  player_dropped: number;
  player_buffer_ms: number;

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
}

export const getPeers = () => invoke<Peer[]>("peers");
export const getDeviceName = () => invoke<string>("device_name");

export const setMinimizeToTray = (enabled: boolean) =>
  invoke<void>("set_minimize_to_tray", { enabled });
