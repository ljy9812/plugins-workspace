import { tag, nfcController } from '@kit.ConnectivityKit';
import { bundleManager } from '@kit.AbilityKit';
import { BusinessError } from '@kit.BasicServicesKit';
import { Plugin, Invoke } from '@tauri/app';

// ─── Types (wire contract — matches guest-js / Android / iOS) ───

interface IsAvailableResponse {
  available: boolean;
}

interface UriFilter {
  scheme?: string;
  host?: string;
  pathPrefix?: string;
}

interface TagKindOptions {
  mimeType?: string;
  uri?: UriFilter;
}

interface NdefKindOptions {
  mimeType?: string;
  uri?: UriFilter;
  techLists?: number[][];
}

// ScanKind wire shape from guest-js mapScanKind(): { "tag": {...} } or { "ndef": {...} }.
interface ScanKindPayload {
  tag?: TagKindOptions;
  ndef?: NdefKindOptions;
}

interface ScanArgs {
  kind?: ScanKindPayload;
  keepSessionAlive?: boolean;
}

interface NfcRecordPayload {
  format: number;
  kind: number[];
  id: number[];
  payload: number[];
}

interface WriteArgs {
  records?: NfcRecordPayload[];
  kind?: ScanKindPayload;
}

interface NfcTagRecordResponse {
  tnf: number;
  kind: number[];
  id: number[];
  payload: number[];
}

// Flat scan response — the Android/iOS/guest-js contract. NOTE: the Rust
// ScanResponse model nests this under `tag` (upstream cross-platform bug that
// breaks the Rust-side Nfc::scan()); the JS invoke path resolves the ArkTS
// JSON verbatim, so the flat shape is what guest-js receives on every platform.
interface ScanResponse {
  id: number[];
  kind: string[];
  records: NfcTagRecordResponse[];
}

interface NfcStatus {
  available: boolean;
  reason: string;
}

// guest-js TechKind enum indices → OHOS tag technology constants
// (JS: IsoDep=0, MifareClassic=1, MifareUltralight=2, Ndef=3,
//  NdefFormatable=4, NfcA=5, NfcB=6, NfcBarcode=7, NfcF=8, NfcV=9)
const JS_TECH_TO_OHOS: number[] = [
  3, // IsoDep → ISO_DEP
  8, // MifareClassic → MIFARE_CLASSIC
  9, // MifareUltralight → MIFARE_ULTRALIGHT
  6, // Ndef → NDEF
  7, // NdefFormatable → NDEF_FORMATABLE
  1, // NfcA → NFC_A
  2, // NfcB → NFC_B
  10, // NfcBarcode → NFC_BARCODE
  4, // NfcF → NFC_F
  5 // NfcV → NFC_V
];

// ─── Plugin Implementation ───

// Tag discovery uses NFC foreground reader mode (tag.on('readerMode'), API
// 11+): the system dispatches TagInfo to the registered callback while the
// ability is in the foreground — no module.json5 skills or Ability intent
// wiring needed, only the ohos.permission.NFC_TAG permission. scan/write keep
// their invoke pending until a tag is discovered (Android Session pattern);
// a keepSessionAlive scan (or a scan-then-write) stores the tag so a later
// write() without `kind` targets it directly.
export class NfcPlugin extends Plugin {
  // Pending scan session (invoke armed until a tag is discovered).
  private pendingScanInvoke: Invoke | null = null;
  private scanKeepAlive: boolean = false;
  private scanTechLists: number[][] | null = null;

  // Pending write session (write with `kind` — armed until tag discovered).
  private pendingWriteInvoke: Invoke | null = null;
  private pendingWriteMessage: tag.NdefMessage | null = null;

  // Tag stored by a keepSessionAlive scan or a completed pending write —
  // later write() calls without `kind` target this tag directly.
  private storedTagInfo: tag.TagInfo | null = null;

  // readerMode registration state (tag.on / tag.off must be paired).
  private readerModeActive: boolean = false;
  private cachedElementName: bundleManager.ElementName | null = null;

  private readerModeCallback = (err: BusinessError, tagInfo: tag.TagInfo): void => {
    if (err) {
      console.error('[NfcPlugin] readerMode error: ' + err.message);
      return; // keep pending invokes armed for the next tap
    }
    console.info('[NfcPlugin] tag discovered: uid=' + tagInfo.uid + ', technology=' + tagInfo.technology);

    if (!this.matchesTechLists(tagInfo)) {
      console.info('[NfcPlugin] tag filtered out by techLists');
      return;
    }

    const scanInvoke: Invoke | null = this.pendingScanInvoke;
    if (scanInvoke != null) {
      const keepAlive: boolean = this.scanKeepAlive;
      this.pendingScanInvoke = null;
      this.scanKeepAlive = false;
      this.scanTechLists = null;
      this.storedTagInfo = keepAlive ? tagInfo : null;
      this.resolveScan(scanInvoke, tagInfo);
      this.updateRegistration();
      return;
    }

    const writeInvoke: Invoke | null = this.pendingWriteInvoke;
    const writeMessage: tag.NdefMessage | null = this.pendingWriteMessage;
    if (writeInvoke != null && writeMessage != null) {
      this.pendingWriteInvoke = null;
      this.pendingWriteMessage = null;
      this.scanTechLists = null;
      this.storedTagInfo = tagInfo; // Android parity: pending-write session keeps the tag
      this.writeTag(tagInfo, writeMessage, writeInvoke);
      return;
    }

    if (this.storedTagInfo != null) {
      this.storedTagInfo = tagInfo; // refresh stored tag on re-tap
      return;
    }

    console.warn('[NfcPlugin] stray readerMode callback — no session');
  };

  getCommands(): Map<string, (invoke: Invoke) => void> {
    const commands: Map<string, (invoke: Invoke) => void> = new Map();
    commands.set('isAvailable', (invoke: Invoke): void => { this.handleIsAvailable(invoke); });
    commands.set('scan', (invoke: Invoke): void => { this.handleScan(invoke); });
    commands.set('write', (invoke: Invoke): void => { this.handleWrite(invoke); });
    return commands;
  }

  // Re-register reader mode when returning to the foreground if a session is
  // still active (Android parity: onResume re-enables foreground dispatch).
  onForeground(): void {
    this.updateRegistration();
  }

  // ─── isAvailable ───

  private handleIsAvailable(invoke: Invoke): void {
    const status: NfcStatus = this.checkNfc();
    const response: IsAvailableResponse = { available: status.available };
    invoke.resolve(JSON.stringify(response));
  }

  private checkNfc(): NfcStatus {
    try {
      const open: boolean = nfcController.isNfcOpen();
      return open
        ? { available: true, reason: '' }
        : { available: false, reason: 'NFC is disabled in device settings' };
    } catch (e) {
      // isNfcOpen throws on devices without NFC capabilities
      return { available: false, reason: 'Device does not have NFC capabilities' };
    }
  }

  // ─── scan ───

  private handleScan(invoke: Invoke): void {
    const status: NfcStatus = this.checkNfc();
    if (!status.available) {
      invoke.reject('NFC unavailable: ' + status.reason);
      return;
    }

    let args: ScanArgs;
    try {
      args = JSON.parse(invoke.parseArgs()) as ScanArgs;
    } catch (e) {
      invoke.reject('Failed to parse scan arguments');
      return;
    }

    // Replace any prior session (Android overwrites it silently; we reject
    // the dangling invokes so no JS promise is left unsettled).
    this.failPendingScan('Scan cancelled by a new scan request');
    this.failPendingWrite('Write cancelled by a new scan request');
    this.storedTagInfo = null;

    this.scanKeepAlive = args.keepSessionAlive === true;
    let techLists: number[][] | null = null;
    if (args.kind != null && args.kind.ndef != null && args.kind.ndef.techLists != null) {
      techLists = args.kind.ndef.techLists;
    }
    this.scanTechLists = techLists;
    this.pendingScanInvoke = invoke;

    // mimeType/uri filters have no readerMode equivalent — discovery is
    // RF-technology based, with techLists applied as a post-filter in the
    // readerMode callback (see matchesTechLists).

    const error: string | null = this.registerReaderMode();
    if (error != null) {
      this.pendingScanInvoke = null;
      this.scanKeepAlive = false;
      this.scanTechLists = null;
      invoke.reject('Failed to start NFC reader mode: ' + error);
    }
  }

  private resolveScan(invoke: Invoke, tagInfo: tag.TagInfo): void {
    let records: tag.NdefRecord[] = [];
    const ndefTag: tag.NdefTag | null = this.tryGetNdef(tagInfo);
    if (ndefTag != null) {
      try {
        ndefTag.connect();
        const message: tag.NdefMessage | null = ndefTag.getNdefMessage();
        if (message != null) {
          records = message.getNdefRecords();
        }
      } catch (e) {
        // Tag without NDEF content — resolve with empty records (Android parity)
        console.warn('[NfcPlugin] no NDEF message: ' + (e as BusinessError).message);
      } finally {
        try {
          ndefTag.resetConnection();
        } catch (e) {
          // ignore close failures
        }
      }
    }

    const response: ScanResponse = {
      id: tagInfo.uid,
      kind: tagInfo.technology.map((t: number): string => this.techName(t)),
      records: records.map((r: tag.NdefRecord): NfcTagRecordResponse => ({
        tnf: r.tnf,
        kind: r.rtdType,
        id: r.id,
        payload: r.payload
      }))
    };
    invoke.resolve(JSON.stringify(response));
  }

  // ─── write ───

  private handleWrite(invoke: Invoke): void {
    const status: NfcStatus = this.checkNfc();
    if (!status.available) {
      invoke.reject('NFC unavailable: ' + status.reason);
      return;
    }

    let args: WriteArgs;
    try {
      args = JSON.parse(invoke.parseArgs()) as WriteArgs;
    } catch (e) {
      invoke.reject('Failed to parse write arguments');
      return;
    }

    if (args.records == null || args.records.length === 0) {
      invoke.reject('Missing `records` for write');
      return;
    }

    let message: tag.NdefMessage;
    try {
      const ndefRecords: tag.NdefRecord[] = [];
      for (const record of args.records) {
        const ndefRecord: tag.NdefRecord = {
          tnf: record.format,
          rtdType: record.kind,
          id: record.id,
          payload: record.payload
        };
        ndefRecords.push(ndefRecord);
      }
      message = tag.ndef.createNdefMessage(ndefRecords);
    } catch (e) {
      invoke.reject('Failed to build NDEF message: ' + (e as BusinessError).message);
      return;
    }

    if (this.storedTagInfo != null) {
      // Write directly to the tag kept alive by a previous scan/write
      this.writeTag(this.storedTagInfo, message, invoke);
      return;
    }

    if (this.pendingScanInvoke != null || this.pendingWriteInvoke != null) {
      invoke.reject('connected tag not found, please wait for it to be available and then call write()');
      return;
    }

    if (args.kind != null) {
      // Scan-then-write: arm a pending write session until a tag is discovered
      this.pendingWriteInvoke = invoke;
      this.pendingWriteMessage = message;
      let techLists: number[][] | null = null;
      if (args.kind.ndef != null && args.kind.ndef.techLists != null) {
        techLists = args.kind.ndef.techLists;
      }
      this.scanTechLists = techLists;

      const error: string | null = this.registerReaderMode();
      if (error != null) {
        this.pendingWriteInvoke = null;
        this.pendingWriteMessage = null;
        this.scanTechLists = null;
        invoke.reject('Failed to start NFC reader mode: ' + error);
      }
      return;
    }

    invoke.reject('Missing `kind` for write');
  }

  private async writeTag(tagInfo: tag.TagInfo, message: tag.NdefMessage, invoke: Invoke): Promise<void> {
    // Primary: already NDEF-formatted tag
    const ndefTag: tag.NdefTag | null = this.tryGetNdef(tagInfo);
    if (ndefTag != null) {
      try {
        ndefTag.connect();
        if (!ndefTag.isNdefWritable()) {
          invoke.reject('NFC tag is read-only');
        } else {
          await ndefTag.writeNdef(message);
          invoke.resolve('');
        }
      } catch (e) {
        invoke.reject("Couldn't write message to NFC tag: " + (e as BusinessError).message);
      } finally {
        try {
          ndefTag.resetConnection();
        } catch (e) {
          // ignore close failures
        }
      }
      return;
    }

    // Fallback: tag that is not yet NDEF-formatted but can be converted
    const formatableTag: tag.NdefFormatableTag | null = this.tryGetNdefFormatable(tagInfo);
    if (formatableTag != null) {
      try {
        formatableTag.connect();
        await formatableTag.format(message);
        invoke.resolve('');
      } catch (e) {
        invoke.reject("Couldn't format tag as Ndef: " + (e as BusinessError).message);
      } finally {
        try {
          formatableTag.resetConnection();
        } catch (e) {
          // ignore close failures
        }
      }
      return;
    }

    invoke.reject("Tag doesn't support Ndef format");
  }

  // ─── readerMode registration ───

  private registerReaderMode(): string | null {
    if (this.readerModeActive) {
      return null;
    }
    const context = this.context;
    if (context == null) {
      return 'plugin context is not available';
    }
    try {
      const elementName: bundleManager.ElementName = {
        deviceId: '',
        bundleName: context.abilityInfo.bundleName,
        moduleName: context.abilityInfo.moduleName,
        abilityName: context.abilityInfo.name
      };
      const discTech: number[] = [tag.NFC_A, tag.NFC_B, tag.NFC_F, tag.NFC_V];
      tag.on('readerMode', elementName, discTech, this.readerModeCallback);
      this.cachedElementName = elementName;
      this.readerModeActive = true;
      console.info('[NfcPlugin] readerMode registered');
      return null;
    } catch (e) {
      const err = e as BusinessError;
      console.error('[NfcPlugin] readerMode registration failed: ' + err.message);
      return err.message;
    }
  }

  private unregisterReaderMode(): void {
    if (!this.readerModeActive) {
      return;
    }
    const elementName: bundleManager.ElementName | null = this.cachedElementName;
    this.readerModeActive = false;
    this.cachedElementName = null;
    if (elementName == null) {
      return;
    }
    try {
      tag.off('readerMode', elementName);
      console.info('[NfcPlugin] readerMode unregistered');
    } catch (e) {
      console.warn('[NfcPlugin] readerMode unregister failed: ' + (e as BusinessError).message);
    }
  }

  private updateRegistration(): void {
    const hasSession: boolean =
      this.pendingScanInvoke != null || this.pendingWriteInvoke != null || this.storedTagInfo != null;
    if (hasSession) {
      const error: string | null = this.registerReaderMode();
      if (error != null) {
        console.warn('[NfcPlugin] foreground re-registration failed: ' + error);
      }
    } else {
      this.unregisterReaderMode();
    }
  }

  // ─── helpers ───

  private failPendingScan(reason: string): void {
    const invoke: Invoke | null = this.pendingScanInvoke;
    if (invoke == null) {
      return;
    }
    this.pendingScanInvoke = null;
    this.scanKeepAlive = false;
    this.scanTechLists = null;
    invoke.reject(reason);
  }

  private failPendingWrite(reason: string): void {
    const invoke: Invoke | null = this.pendingWriteInvoke;
    if (invoke == null) {
      return;
    }
    this.pendingWriteInvoke = null;
    this.pendingWriteMessage = null;
    invoke.reject(reason);
  }

  private tryGetNdef(tagInfo: tag.TagInfo): tag.NdefTag | null {
    try {
      return tag.getNdef(tagInfo);
    } catch (e) {
      // Tag does not support the NDEF technology
      return null;
    }
  }

  private tryGetNdefFormatable(tagInfo: tag.TagInfo): tag.NdefFormatableTag | null {
    try {
      return tag.getNdefFormatable(tagInfo);
    } catch (e) {
      // Tag is not NDEF-formatable
      return null;
    }
  }

  // techLists post-filter: a tag matches if it supports ALL technologies of
  // at least one tech-list (Android AND-within-list / OR-across-lists
  // semantics; readerMode discTech itself can only express a flat OR of RF
  // technologies).
  private matchesTechLists(tagInfo: tag.TagInfo): boolean {
    const techLists: number[][] | null = this.scanTechLists;
    if (techLists == null || techLists.length === 0) {
      return true;
    }
    for (const list of techLists) {
      if (list.length === 0) {
        continue;
      }
      let allMatch: boolean = true;
      for (const tech of list) {
        let techNumber: number = -1;
        if (tech >= 0 && tech < JS_TECH_TO_OHOS.length) {
          techNumber = JS_TECH_TO_OHOS[tech];
        }
        if (techNumber < 0 || !tagInfo.technology.includes(techNumber)) {
          allMatch = false;
          break;
        }
      }
      if (allMatch) {
        return true;
      }
    }
    return false;
  }

  private techName(tech: number): string {
    switch (tech) {
      case 1:
        return 'NfcA';
      case 2:
        return 'NfcB';
      case 3:
        return 'IsoDep';
      case 4:
        return 'NfcF';
      case 5:
        return 'NfcV';
      case 6:
        return 'Ndef';
      case 7:
        return 'NdefFormatable';
      case 8:
        return 'MifareClassic';
      case 9:
        return 'MifareUltralight';
      case 10:
        return 'NfcBarcode';
      default:
        return 'Unknown';
    }
  }
}

export default NfcPlugin;
