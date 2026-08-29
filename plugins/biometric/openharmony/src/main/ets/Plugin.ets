import { userAuth } from '@kit.UserAuthenticationKit';
import { BusinessError } from '@kit.BasicServicesKit';
import { Plugin, Invoke } from '@tauri/app';

// ─── Types ───

interface AuthArgs {
  reason: string;
  allowDeviceCredential: boolean;
  cancelTitle: string | null;
  fallbackTitle: string | null;
  title: string | null;
  subtitle: string | null;
  confirmationRequired: boolean | null;
}

interface StatusResult {
  isAvailable: boolean;
  biometryType: number; // 0=None, 1=TouchID (fingerprint), 2=FaceID (face)
  error: string | null;
  errorCode: string | null;
}

// ─── Plugin Implementation ───

export class BiometricPlugin extends Plugin {
  getCommands(): Map<string, (invoke: Invoke) => void> {
    const commands: Map<string, (invoke: Invoke) => void> = new Map();
    commands.set('status', (invoke: Invoke): void => { this.handleStatus(invoke); });
    commands.set('authenticate', (invoke: Invoke): void => { this.handleAuthenticate(invoke); });
    return commands;
  }

  // ─── status ───

  private handleStatus(invoke: Invoke): void {
    this.doStatus(invoke);
  }

  private async doStatus(invoke: Invoke): Promise<void> {
    const result: StatusResult = {
      isAvailable: false,
      biometryType: 0,
      error: null,
      errorCode: null
    };

    // Check face authentication availability
    let faceAvailable = false;
    try {
      userAuth.getAvailableStatus(userAuth.UserAuthType.FACE, userAuth.AuthTrustLevel.ATL1);
      faceAvailable = true;
    } catch (e) {
      faceAvailable = false;
    }

    // Check fingerprint authentication availability
    let fingerprintAvailable = false;
    try {
      userAuth.getAvailableStatus(userAuth.UserAuthType.FINGERPRINT, userAuth.AuthTrustLevel.ATL1);
      fingerprintAvailable = true;
    } catch (e) {
      fingerprintAvailable = false;
    }

    if (faceAvailable) {
      result.isAvailable = true;
      result.biometryType = 2; // FaceID
    } else if (fingerprintAvailable) {
      result.isAvailable = true;
      result.biometryType = 1; // TouchID (fingerprint)
    } else {
      result.isAvailable = false;
      result.biometryType = 0;
      result.error = 'biometryNotAvailable';
      result.errorCode = 'biometryNotAvailable';
    }

    invoke.resolve(JSON.stringify(result));
  }

  // ─── authenticate ───

  private handleAuthenticate(invoke: Invoke): void {
    this.doAuthenticate(invoke);
  }

  private async doAuthenticate(invoke: Invoke): Promise<void> {
    try {
      const args: AuthArgs = JSON.parse(invoke.parseArgs()) as AuthArgs;

      // Widget auth requires EXACTLY ONE biometric authType — passing both
      // FACE + FINGERPRINT is rejected with "authType check fail:0" → ret:8
      // ("check permission and auth widget param failed"). Probe which type
      // the device actually supports (mirrors doStatus) and use the first
      // available one; the system auto-falls-back to PIN on biometric failure.
      let authType: userAuth.UserAuthType;
      let faceAvailable = false;
      try {
        userAuth.getAvailableStatus(userAuth.UserAuthType.FACE, userAuth.AuthTrustLevel.ATL1);
        faceAvailable = true;
      } catch (e) {
        faceAvailable = false;
      }
      let fingerprintAvailable = false;
      try {
        userAuth.getAvailableStatus(userAuth.UserAuthType.FINGERPRINT, userAuth.AuthTrustLevel.ATL1);
        fingerprintAvailable = true;
      } catch (e) {
        fingerprintAvailable = false;
      }
      if (faceAvailable) {
        authType = userAuth.UserAuthType.FACE;
      } else if (fingerprintAvailable) {
        authType = userAuth.UserAuthType.FINGERPRINT;
      } else {
        invoke.reject('biometryNotAvailable');
        return;
      }

      const authParam: userAuth.AuthParam = {
        challenge: new Uint8Array([0]),
        authType: [authType],
        authTrustLevel: userAuth.AuthTrustLevel.ATL1
      };

      // Widget auth (API 18+) needs widgetParam.uiContext to display the auth
      // dialog in modal-application mode bound to this Ability's window. Without
      // it the system logs "BeginWidgetAuth has context: 0 / AuthWidget fail
      // ret:8" and no popup appears. UIAbilityContext extends Context, so
      // this.context (set by PluginManager.setContext) satisfies the
      // widgetParam.uiContext?: Context field directly.
      if (!this.context) {
        invoke.reject('authenticate failed: Plugin context not initialized');
        return;
      }

      const widgetParam: userAuth.WidgetParam = {
        title: args.title != null ? args.title : (args.reason != null ? args.reason : 'Authentication'),
        uiContext: this.context
      };

      const authInstance = userAuth.getUserAuthInstance(authParam, widgetParam);

      // IAuthCallback is an interface (onResult method), not a function type —
      // see @ohos.userIAM.userAuth.d.ts. There is no 'error' event; async
      // failures (cancel, lockout, …) arrive here via result.result, and
      // synchronous errors from start() are caught by the outer try/catch.
      authInstance.on('result', {
        onResult: (result: userAuth.UserAuthResult): void => {
          if (result.result === userAuth.UserAuthResultCode.SUCCESS) {
            invoke.resolve('');
          } else {
            const errorCode = this.mapResultCode(result.result);
            invoke.reject('Authentication failed: ' + errorCode);
          }
        }
      });

      authInstance.start();
    } catch (e) {
      const err = e as BusinessError;
      console.error('[BiometricPlugin] authenticate failed: code=' + err.code + ', message=' + err.message);
      invoke.reject('authenticate failed: ' + err.message);
    }
  }

  private mapResultCode(code: number): string {
    // Map OHOS UserAuthResultCode to Tauri errorCode strings where possible
    if (code === userAuth.UserAuthResultCode.SUCCESS) {
      return 'success';
    }
    switch (code) {
      case 12500001:
        return 'authenticationFailed';
      case 12500002:
        return 'userCancel';
      case 12500003:
        return 'userFallback';
      case 12500004:
        return 'biometryLockout';
      case 12500005:
        return 'biometryNotEnrolled';
      case 12500006:
        return 'biometryNotAvailable';
      case 12500007:
        return 'systemCancel';
      case 12500009:
        return 'appCancel';
      default:
        return 'authenticationFailed';
    }
  }
}

export default BiometricPlugin;
