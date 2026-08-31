import { nfcController } from '@kit.ConnectivityKit';
import { BusinessError } from '@kit.BasicServicesKit';
import { Plugin, Invoke } from '@tauri/app';

// ─── Types ───

interface IsAvailableResponse {
  available: boolean;
}

interface NfcTagRecord {
  tnf: number;
  kind: number[];
  id: number[];
  payload: number[];
}

interface NfcTag {
  id: string;
  kind: string;
  records: NfcTagRecord[];
}

interface ScanResponse {
  tag: NfcTag;
}

interface WriteRequest {
  records: NfcRecord[];
}

interface NfcRecord {
  format: number;
  kind: number[];
  id: number[];
  payload: number[];
}

// ─── Plugin Implementation ───

// NOTE: OHOS NFC tag reading requires a tag to be discovered via an Ability
// intent (ACTION_TAG_DISCOVERED / tech-discovered). The mobile plugin Plugin
// base class has no access to the Ability intent lifecycle, so full scan/write
// sessions are not possible from within this plugin alone. isAvailable checks
// the NFC controller state. scan/write reject with a clear message documenting
// that the host app must integrate OHOS NFC foreground dispatch.

export class NfcPlugin extends Plugin {
  getCommands(): Map<string, (invoke: Invoke) => void> {
    const commands: Map<string, (invoke: Invoke) => void> = new Map();
    commands.set('isAvailable', (invoke: Invoke): void => { this.handleIsAvailable(invoke); });
    commands.set('scan', (invoke: Invoke): void => { this.handleScan(invoke); });
    commands.set('write', (invoke: Invoke): void => { this.handleWrite(invoke); });
    return commands;
  }

  // ─── isAvailable ───

  private handleIsAvailable(invoke: Invoke): void {
    this.doIsAvailable(invoke);
  }

  private async doIsAvailable(invoke: Invoke): Promise<void> {
    try {
      // nfcController.isNfcAvailable() returns whether the device supports NFC.
      // isNfcOpen() returns the current open/close state.
      let supported = true;
      let enabled = false;
      try {
        enabled = nfcController.isNfcOpen();
      } catch (e) {
        // isNfcOpen may throw if NFC is not supported
        supported = false;
      }
      const response: IsAvailableResponse = { available: supported && enabled };
      invoke.resolve(JSON.stringify(response));
    } catch (e) {
      const err = e as BusinessError;
      console.error('[NfcPlugin] isAvailable failed: ' + err.message);
      invoke.resolve(JSON.stringify({ available: false } as IsAvailableResponse));
    }
  }

  // ─── scan ───

  private handleScan(invoke: Invoke): void {
    // NFC tag discovery on OHOS requires Ability-level intent handling
    // (ACTION_TAG_DISCOVERED) which is outside the mobile plugin scope.
    // The host app must implement foreground dispatch and feed the tag info.
    invoke.reject('NFC scan is not supported from the plugin on OHOS: tag discovery requires Ability-level intent (ACTION_TAG_DISCOVERED) integration. See plugin design notes.');
  }

  // ─── write ───

  private handleWrite(invoke: Invoke): void {
    invoke.reject('NFC write is not supported from the plugin on OHOS: tag discovery requires Ability-level intent (ACTION_TAG_DISCOVERED) integration. See plugin design notes.');
  }
}

export default NfcPlugin;
