import { vibrator } from '@kit.SensorServiceKit';
import { BusinessError } from '@kit.BasicServicesKit';
import { Plugin, Invoke } from '@tauri/app';

// ─── Types ───

interface VibrateArgs {
  duration: number;
}

interface ImpactFeedbackArgs {
  style: string;
}

interface NotificationFeedbackArgs {
  type: string;
}

// ─── Plugin Implementation ───

// Maps Tauri ImpactFeedbackStyle (serde camelCase: light/medium/heavy/soft/rigid)
// to OHOS preset vibration effect IDs.
function impactStyleToEffectId(style: string): string {
  switch (style) {
    case 'light':
      return 'haptic.effect.soft';
    case 'heavy':
      return 'haptic.effect.rigid';
    case 'soft':
      return 'haptic.effect.soft';
    case 'rigid':
      return 'haptic.effect.rigid';
    case 'medium':
    default:
      return 'haptic.effect.light';
  }
}

// Maps Tauri NotificationFeedbackType (serde camelCase: success/warning/error)
// to OHOS preset vibration effect IDs.
function notificationTypeToEffectId(type: string): string {
  switch (type) {
    case 'success':
      return 'haptic.effect.rigid';
    case 'warning':
      return 'haptic.effect.light';
    case 'error':
      return 'haptic.effect.heavy';
    default:
      return 'haptic.effect.light';
  }
}

export class HapticsPlugin extends Plugin {
  getCommands(): Map<string, (invoke: Invoke) => void> {
    const commands: Map<string, (invoke: Invoke) => void> = new Map();
    commands.set('vibrate', (invoke: Invoke): void => { this.handleVibrate(invoke); });
    commands.set('impactFeedback', (invoke: Invoke): void => { this.handleImpactFeedback(invoke); });
    commands.set('notificationFeedback', (invoke: Invoke): void => { this.handleNotificationFeedback(invoke); });
    commands.set('selectionFeedback', (invoke: Invoke): void => { this.handleSelectionFeedback(invoke); });
    return commands;
  }

  // ─── vibrate (duration in ms) ───

  private handleVibrate(invoke: Invoke): void {
    this.doVibrate(invoke);
  }

  private async doVibrate(invoke: Invoke): Promise<void> {
    try {
      const args: VibrateArgs = JSON.parse(invoke.parseArgs()) as VibrateArgs;
      const duration = args.duration > 0 ? args.duration : 500;
      await vibrator.startVibration(
        { type: 'time', duration: duration } as vibrator.VibrateEffect,
        { usage: 'unknown' } as vibrator.VibrateAttribute
      );
      invoke.resolve('');
    } catch (e) {
      const err = e as BusinessError;
      console.error('[HapticsPlugin] vibrate failed: code=' + err.code + ', message=' + err.message);
      invoke.reject('vibrate failed: ' + err.message);
    }
  }

  // ─── impactFeedback (style) ───

  private handleImpactFeedback(invoke: Invoke): void {
    this.doImpactFeedback(invoke);
  }

  private async doImpactFeedback(invoke: Invoke): Promise<void> {
    try {
      const args: ImpactFeedbackArgs = JSON.parse(invoke.parseArgs()) as ImpactFeedbackArgs;
      const effectId = impactStyleToEffectId(args.style);
      const supported = await vibrator.isSupportEffect(effectId);
      if (!supported) {
        // Fallback to a short time vibration
        await vibrator.startVibration(
          { type: 'time', duration: 30 } as vibrator.VibrateEffect,
          { usage: 'unknown' } as vibrator.VibrateAttribute
        );
        invoke.resolve('');
        return;
      }
      await vibrator.startVibration(
        { type: 'preset', effectId: effectId, count: 1, intensity: 50 } as vibrator.VibrateEffect,
        { usage: 'unknown' } as vibrator.VibrateAttribute
      );
      invoke.resolve('');
    } catch (e) {
      const err = e as BusinessError;
      console.error('[HapticsPlugin] impactFeedback failed: ' + err.message);
      invoke.reject('impactFeedback failed: ' + err.message);
    }
  }

  // ─── notificationFeedback (type) ───

  private handleNotificationFeedback(invoke: Invoke): void {
    this.doNotificationFeedback(invoke);
  }

  private async doNotificationFeedback(invoke: Invoke): Promise<void> {
    try {
      const args: NotificationFeedbackArgs = JSON.parse(invoke.parseArgs()) as NotificationFeedbackArgs;
      const effectId = notificationTypeToEffectId(args.type);
      const supported = await vibrator.isSupportEffect(effectId);
      if (!supported) {
        await vibrator.startVibration(
          { type: 'time', duration: 50 } as vibrator.VibrateEffect,
          { usage: 'unknown' } as vibrator.VibrateAttribute
        );
        invoke.resolve('');
        return;
      }
      await vibrator.startVibration(
        { type: 'preset', effectId: effectId, count: 1, intensity: 60 } as vibrator.VibrateEffect,
        { usage: 'unknown' } as vibrator.VibrateAttribute
      );
      invoke.resolve('');
    } catch (e) {
      const err = e as BusinessError;
      console.error('[HapticsPlugin] notificationFeedback failed: ' + err.message);
      invoke.reject('notificationFeedback failed: ' + err.message);
    }
  }

  // ─── selectionFeedback ───

  private handleSelectionFeedback(invoke: Invoke): void {
    this.doSelectionFeedback(invoke);
  }

  private async doSelectionFeedback(invoke: Invoke): Promise<void> {
    try {
      const effectId = 'haptic.effect.soft';
      const supported = await vibrator.isSupportEffect(effectId);
      if (!supported) {
        await vibrator.startVibration(
          { type: 'time', duration: 15 } as vibrator.VibrateEffect,
          { usage: 'unknown' } as vibrator.VibrateAttribute
        );
        invoke.resolve('');
        return;
      }
      await vibrator.startVibration(
        { type: 'preset', effectId: effectId, count: 1, intensity: 40 } as vibrator.VibrateEffect,
        { usage: 'unknown' } as vibrator.VibrateAttribute
      );
      invoke.resolve('');
    } catch (e) {
      const err = e as BusinessError;
      console.error('[HapticsPlugin] selectionFeedback failed: ' + err.message);
      invoke.reject('selectionFeedback failed: ' + err.message);
    }
  }
}

export default HapticsPlugin;
