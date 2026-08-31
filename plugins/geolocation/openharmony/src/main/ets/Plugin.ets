import { geoLocationManager } from '@kit.LocationKit';
import { abilityAccessCtrl, bundleManager, Permissions, Want } from '@kit.AbilityKit';
import { BusinessError } from '@kit.BasicServicesKit';
import { Plugin, Invoke } from '@tauri/app';

// ─── Types ───

interface PositionOptions {
  enableHighAccuracy: boolean;
  timeout: number;
  maximumAge: number;
}

interface WatchPayload {
  options: PositionOptions;
  channel: string; // Tauri IPC channel id wire-format (e.g. "__CHANNEL__:123")
}

interface ClearWatchPayload {
  channelId: number;
}

interface Coordinates {
  latitude: number;
  longitude: number;
  accuracy: number;
  altitudeAccuracy: number | null;
  altitude: number | null;
  speed: number | null;
  heading: number | null;
}

interface Position {
  timestamp: number;
  coords: Coordinates;
}

interface PermissionStatus {
  location: string;
  coarseLocation: string;
}

interface RequestPermissionsArgs {
  permissions: string[] | null;
}

// ─── Plugin Implementation ───

const PERMISSION_FINE: Permissions = 'ohos.permission.LOCATION';
const PERMISSION_APPROX: Permissions = 'ohos.permission.APPROXIMATELY_LOCATION';

function permissionStateToString(state: number): string {
  // abilityAccessCtrl.GrantStatus / authResults: 0 = granted, -1 = denied
  if (state === 0) {
    return 'granted';
  }
  if (state === -1) {
    return 'denied';
  }
  return 'prompt';
}

export class GeolocationPlugin extends Plugin {
  private watchCallback: ((location: geoLocationManager.Location) => void) | null = null;
  private watchRequest: geoLocationManager.ContinuousLocationRequest | null = null;
  private watchChannelId: number | null = null;

  // ─── Permission request fallback state ───
  // Tracks a pending requestPermissionsFromUser call so that it can be
  // settled via one of three paths if the original Promise never resolves
  // (system permission dialog causes ability onBackground → event loop freeze).
  private pendingPermissionInvoke: Invoke | null = null;
  private permissionSettled: boolean = false;
  private permissionStateCallback: ((info: abilityAccessCtrl.PermissionStateChangeInfo) => void) | null = null;
  private permissionStateList: Array<Permissions> = [];
  private permissionTimeoutId: number = -1;

  getCommands(): Map<string, (invoke: Invoke) => void> {
    const commands: Map<string, (invoke: Invoke) => void> = new Map();
    commands.set('getCurrentPosition', (invoke: Invoke): void => { this.handleGetCurrentPosition(invoke); });
    commands.set('watchPosition', (invoke: Invoke): void => { this.handleWatchPosition(invoke); });
    commands.set('clearWatch', (invoke: Invoke): void => { this.handleClearWatch(invoke); });
    commands.set('checkPermissions', (invoke: Invoke): void => { this.handleCheckPermissions(invoke); });
    commands.set('requestPermissions', (invoke: Invoke): void => { this.handleRequestPermissions(invoke); });
    commands.set('openLocationSettings', (invoke: Invoke): void => { this.handleOpenLocationSettings(invoke); });
    return commands;
  }

  /**
   * Called by PluginManager.notifyForeground() when the host ability returns
   * to the foreground. If there is a pending permission request that hasn't
   * been settled yet, settle it now by reading the real permission state.
   */
  onForeground(): void {
    if (this.pendingPermissionInvoke != null && !this.permissionSettled) {
      this.settlePermissions('onForeground');
    }
  }

  // ─── getCurrentPosition ───

  private handleGetCurrentPosition(invoke: Invoke): void {
    this.doGetCurrentPosition(invoke);
  }

  private async doGetCurrentPosition(invoke: Invoke): Promise<void> {
    try {
      const argsStr = invoke.parseArgs();
      let options: PositionOptions;
      if (argsStr === '' || argsStr === 'null' || argsStr === '{}') {
        options = { enableHighAccuracy: false, timeout: 10000, maximumAge: 0 } as PositionOptions;
      } else {
        options = JSON.parse(argsStr) as PositionOptions;
      }

      const priority = options.enableHighAccuracy
        ? geoLocationManager.LocatingPriority.PRIORITY_ACCURACY
        : geoLocationManager.LocatingPriority.PRIORITY_LOCATING_SPEED;

      const request: geoLocationManager.SingleLocationRequest = {
        locatingPriority: priority,
        locatingTimeoutMs: options.timeout > 0 ? options.timeout : 10000
      };

      const location = await geoLocationManager.getCurrentLocation(request);
      const position = this.locationToPosition(location);
      invoke.resolve(JSON.stringify(position));
    } catch (e) {
      const err = e as BusinessError;
      console.error('[GeolocationPlugin] getCurrentPosition failed: code=' + err.code + ', message=' + err.message);
      invoke.reject('getCurrentPosition failed: ' + err.message);
    }
  }

  private locationToPosition(location: geoLocationManager.Location): Position {
    const coords: Coordinates = {
      latitude: location.latitude,
      longitude: location.longitude,
      accuracy: location.accuracy,
      altitudeAccuracy: location.accuracy, // OHOS does not expose separate altitude accuracy
      altitude: location.altitude,
      speed: location.speed,
      heading: location.direction
    };
    return {
      timestamp: location.timeStamp,
      coords: coords
    };
  }

  // ─── watchPosition ───

  private handleWatchPosition(invoke: Invoke): void {
    this.doWatchPosition(invoke);
  }

  private async doWatchPosition(invoke: Invoke): Promise<void> {
    try {
      const payload: WatchPayload = JSON.parse(invoke.parseArgs()) as WatchPayload;
      const options = payload.options;

      // Parse the Tauri IPC channel id from the wire-format string.
      const channelId = Plugin.parseChannelId(payload.channel);
      if (channelId == null) {
        invoke.reject('watchPosition: invalid channel id: ' + payload.channel);
        return;
      }

      // Clean up any previous watch
      this.clearWatchInternal();

      this.watchChannelId = channelId;

      const request: geoLocationManager.ContinuousLocationRequest = {
        interval: 1,
        locationScenario: geoLocationManager.UserActivityScenario.NAVIGATION
      };
      this.watchRequest = request;

      const self = this;
      const callback = (location: geoLocationManager.Location): void => {
        const cid = self.watchChannelId;
        if (cid != null) {
          const position = self.locationToPosition(location);
          self.emit(cid, position);
        }
      };
      this.watchCallback = callback;

      geoLocationManager.on('locationChange', request, callback);
      invoke.resolve('');
    } catch (e) {
      const err = e as BusinessError;
      console.error('[GeolocationPlugin] watchPosition failed: ' + err.message);
      invoke.reject('watchPosition failed: ' + err.message);
    }
  }

  // ─── clearWatch ───

  private handleClearWatch(invoke: Invoke): void {
    this.clearWatchInternal();
    invoke.resolve('');
  }

  private clearWatchInternal(): void {
    if (this.watchCallback != null) {
      try {
        geoLocationManager.off('locationChange', this.watchCallback);
      } catch (e) {
        console.warn('[GeolocationPlugin] clearWatch off failed: ' + (e as BusinessError).message);
      }
      this.watchCallback = null;
    }
    this.watchRequest = null;
    this.watchChannelId = null;
  }

  // ─── checkPermissions ───

  private handleCheckPermissions(invoke: Invoke): void {
    this.doCheckPermissions(invoke);
  }

  private async doCheckPermissions(invoke: Invoke): Promise<void> {
    try {
      const status = this.queryPermissionStatus();
      invoke.resolve(JSON.stringify(status));
    } catch (e) {
      console.warn('[GeolocationPlugin] checkPermissions failed: ' + (e as BusinessError).message);
      invoke.resolve(JSON.stringify({ location: 'prompt', coarseLocation: 'prompt' } as PermissionStatus));
    }
  }

  private queryPermissionStatus(): PermissionStatus {
    let fineState = 'prompt';
    let approxState = 'prompt';
    try {
      const info = bundleManager.getBundleInfoForSelfSync(
        bundleManager.BundleFlag.GET_BUNDLE_INFO_WITH_APPLICATION
      );
      const token = info.appInfo.accessTokenId;
      const mgr = abilityAccessCtrl.createAtManager();
      fineState = permissionStateToString(mgr.checkAccessTokenSync(token, PERMISSION_FINE));
      approxState = permissionStateToString(mgr.checkAccessTokenSync(token, PERMISSION_APPROX));
    } catch (e) {
      console.warn('[GeolocationPlugin] permission query failed: ' + (e as BusinessError).message);
    }
    return { location: fineState, coarseLocation: approxState } as PermissionStatus;
  }

  // ─── requestPermissions ───

  private handleRequestPermissions(invoke: Invoke): void {
    this.doRequestPermissions(invoke);
  }

  private async doRequestPermissions(invoke: Invoke): Promise<void> {
    // requestPermissionsFromUser needs the UIAbilityContext, injected by
    // PluginManager.setContext() from the host EntryAbility. When null
    // (e.g. standalone shim build), degrade to returning the current state.
    if (this.context == null) {
      const status = this.queryPermissionStatus();
      invoke.resolve(JSON.stringify(status));
      return;
    }
    this.requestPermissionsWithFallback(invoke);
  }

  /**
   * Three-path permission request with fallback settlement:
   *   Path 1 (onForeground): ability returns to foreground after dialog → settle.
   *   Path 2 (selfPermissionStateChange event): system fires permission state
   *       change callback → settle.
   *   Path 3 (setTimeout 60s): safety net for foreground-only scenarios → settle.
   *   Original Promise .then/.catch also settles (if it ever resolves).
   * All settlement paths read the real permission state via
   * queryPermissionStatus(), not the possibly-stale authResults array.
   */
  private requestPermissionsWithFallback(invoke: Invoke): void {
    // Initialize pending state
    this.pendingPermissionInvoke = invoke;
    this.permissionSettled = false;
    this.permissionStateList = [PERMISSION_FINE, PERMISSION_APPROX];

    // Path 2: Register selfPermissionStateChange listener (API 18+).
    // Wrap in try/catch — duplicate registration throws 12100004.
    try {
      const mgr = abilityAccessCtrl.createAtManager();
      const callback = (info: abilityAccessCtrl.PermissionStateChangeInfo): void => {
        this.settlePermissions('selfPermissionStateChange');
      };
      this.permissionStateCallback = callback;
      mgr.on('selfPermissionStateChange', this.permissionStateList, callback);
    } catch (e) {
      console.warn('[GEO] on selfPermissionStateChange failed: ' + (e as BusinessError).message);
      this.permissionStateCallback = null;
    }

    // Path 3: setTimeout 60s safety net (only effective while in foreground).
    this.permissionTimeoutId = setTimeout((): void => {
      console.warn('[GEO] permission request safety-net timeout (60s) fired; settling with polled state');
      this.settlePermissions('timeout');
    }, 60000);

    // Original path: fire requestPermissionsFromUser as fire-and-forget.
    // If the Promise never resolves (event loop frozen during dialog),
    // the other paths handle settlement.
    this.firePermissionRequest();
  }

  private async firePermissionRequest(): Promise<void> {
    try {
      const mgr = abilityAccessCtrl.createAtManager();
      await mgr.requestPermissionsFromUser(this.context!, [PERMISSION_FINE, PERMISSION_APPROX]);
      this.settlePermissions('promise-resolved');
    } catch (e) {
      console.warn('[GEO] promise rejected: ' + (e as BusinessError).message);
      this.settlePermissions('promise-rejected');
    }
  }

  /**
   * Settles the pending permission request (if any) using the real permission
   * state read via queryPermissionStatus(). Idempotent — the `settled` flag
   * ensures only the first path to call this actually resolves the invoke.
   */
  private settlePermissions(source: string): void {
    if (this.permissionSettled) {
      return;
    }
    this.permissionSettled = true;

    // Cleanup listeners and timeout
    this.cleanupPermissionListeners();

    // Resolve with current permission status (reads real state, not cached authResults).
    // selfPermissionStateChange can fire BEFORE the grant is committed and visible
    // to checkAccessTokenSync, so poll for a short window instead of a single read.
    const invoke = this.pendingPermissionInvoke;
    this.pendingPermissionInvoke = null;
    if (invoke != null) {
      this.resolveWithPolledStatus(invoke, 0);
    }
  }

  /**
   * Resolves the pending permission invoke with a polled permission read.
   * selfPermissionStateChange can fire BEFORE the grant is committed and
   * visible to checkAccessTokenSync (observed on device: event change=1 but
   * sync query still 'denied'). First read is immediate (fast path: already
   * granted resolves with no delay); if some requested permission is not yet
   * granted, re-read every 300ms up to 6 retries (~1.8s), then resolve with
   * the last read regardless.
   */
  private resolveWithPolledStatus(invoke: Invoke, attempt: number): void {
    const status = this.queryPermissionStatus();
    const allGranted: boolean = status.location === 'granted' && status.coarseLocation === 'granted';
    if (allGranted || attempt >= 6) {
      invoke.resolve(JSON.stringify(status));
      return;
    }
    setTimeout((): void => {
      this.resolveWithPolledStatus(invoke, attempt + 1);
    }, 300);
  }

  private cleanupPermissionListeners(): void {
    // off the selfPermissionStateChange listener (must pass same callback + permissionList)
    if (this.permissionStateCallback != null) {
      try {
        const mgr = abilityAccessCtrl.createAtManager();
        mgr.off('selfPermissionStateChange', this.permissionStateList, this.permissionStateCallback);
      } catch (e) {
        console.warn('[GEO] off selfPermissionStateChange failed: ' + (e as BusinessError).message);
      }
      this.permissionStateCallback = null;
    }
    // clear the timeout
    if (this.permissionTimeoutId !== -1) {
      clearTimeout(this.permissionTimeoutId);
      this.permissionTimeoutId = -1;
    }
  }

  // ─── openLocationSettings ───

  private handleOpenLocationSettings(invoke: Invoke): void {
    this.doOpenLocationSettings(invoke);
  }

  private async doOpenLocationSettings(invoke: Invoke): Promise<void> {
    if (this.context == null) {
      invoke.reject('openLocationSettings: context not available');
      return;
    }
    // 1) System location service master-switch page (Huawei settings app).
    //    App-level permission alone is not enough: with the master switch off,
    //    locManager APIs fail with BusinessError 3301100.
    try {
      const want: Want = {
        bundleName: 'com.huawei.hmos.settings',
        abilityName: 'com.huawei.hmos.settings.MainAbility',
        uri: 'location_manager_settings'
      };
      await this.context.startAbility(want);
      // Rust command signature is unit: resolve('') deserializes as Null → ()
      // (same convention as clearWatch); a string payload fails unit deserialization.
      invoke.resolve('');
      return;
    } catch (e) {
      console.warn('[GeolocationPlugin] open location settings failed: ' + (e as BusinessError).message);
    }
    // 2) Fallback: this app's details page (has a permission-manager entry).
    try {
      const bundleName: string = this.context.abilityInfo.bundleName;
      const want: Want = {
        bundleName: 'com.huawei.hmos.settings',
        abilityName: 'com.huawei.hmos.settings.MainAbility',
        uri: 'application_info_settings?package=' + bundleName
      };
      await this.context.startAbility(want);
      invoke.resolve('');
    } catch (e) {
      const err = e as BusinessError;
      console.error('[GeolocationPlugin] open app details failed: code=' + err.code + ', message=' + err.message);
      invoke.reject('openLocationSettings failed: ' + err.message);
    }
  }
}

export default GeolocationPlugin;
