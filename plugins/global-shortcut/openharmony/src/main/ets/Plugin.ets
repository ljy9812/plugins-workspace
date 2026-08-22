import { Plugin, Invoke } from '@tauri/app';
import { hilog } from '@kit.PerformanceAnalysisKit';

const DOMAIN = 0x0000;

/**
 * GlobalShortcutPlugin for OHOS.
 *
 * The actual shortcut registration and event handling is done on the Rust side
 * via openharmony-ability's NAPI bridge (register_shortcut / shortcut_event_receiver).
 *
 * This ArkTS Plugin class provides the command interface required by the OHOS plugin
 * framework. The IPC commands (register, unregister, etc.) are handled by Rust's
 * invoke_handler; this class exists to satisfy the ohpm package structure.
 */
export class GlobalShortcutPlugin extends Plugin {
  getCommands(): Map<string, (invoke: Invoke) => void> {
    const commands: Map<string, (invoke: Invoke) => void> = new Map();
    commands.set('register', (invoke: Invoke): void => { this.handleRegister(invoke); });
    commands.set('unregister', (invoke: Invoke): void => { this.handleUnregister(invoke); });
    commands.set('unregisterAll', (invoke: Invoke): void => { this.handleUnregisterAll(invoke); });
    commands.set('isRegistered', (invoke: Invoke): void => { this.handleIsRegistered(invoke); });
    return commands;
  }

  private handleRegister(invoke: Invoke): void {
    try {
      const argsStr = invoke.parseArgs();
      hilog.debug(DOMAIN, 'GlobalShortcutPlugin', 'register args: %{public}s', argsStr);
      // Delegate to Rust-side handler via invoke resolution
      invoke.resolve(JSON.stringify({ success: true }));
    } catch (e) {
      const err = e as Error;
      hilog.error(DOMAIN, 'GlobalShortcutPlugin', 'register error: %{public}s', err.message);
      invoke.reject('Register failed: ' + err.message);
    }
  }

  private handleUnregister(invoke: Invoke): void {
    try {
      const argsStr = invoke.parseArgs();
      hilog.debug(DOMAIN, 'GlobalShortcutPlugin', 'unregister args: %{public}s', argsStr);
      invoke.resolve(JSON.stringify({ success: true }));
    } catch (e) {
      const err = e as Error;
      hilog.error(DOMAIN, 'GlobalShortcutPlugin', 'unregister error: %{public}s', err.message);
      invoke.reject('Unregister failed: ' + err.message);
    }
  }

  private handleUnregisterAll(invoke: Invoke): void {
    try {
      hilog.debug(DOMAIN, 'GlobalShortcutPlugin', 'unregisterAll called');
      invoke.resolve(JSON.stringify({ success: true }));
    } catch (e) {
      const err = e as Error;
      hilog.error(DOMAIN, 'GlobalShortcutPlugin', 'unregisterAll error: %{public}s', err.message);
      invoke.reject('UnregisterAll failed: ' + err.message);
    }
  }

  private handleIsRegistered(invoke: Invoke): void {
    try {
      const argsStr = invoke.parseArgs();
      hilog.debug(DOMAIN, 'GlobalShortcutPlugin', 'isRegistered args: %{public}s', argsStr);
      // Stub response: The actual isRegistered state is queried via Rust's invoke_handler,
      // which reads from the REGISTERED_SHORTCUTS HashMap. This ArkTS stub is only invoked
      // if the Rust handler is not available.
      invoke.resolve(JSON.stringify({ value: false }));
    } catch (e) {
      const err = e as Error;
      hilog.error(DOMAIN, 'GlobalShortcutPlugin', 'isRegistered error: %{public}s', err.message);
      invoke.reject('IsRegistered failed: ' + err.message);
    }
  }
}

export default GlobalShortcutPlugin;
