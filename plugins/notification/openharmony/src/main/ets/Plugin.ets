import { notificationManager } from '@kit.NotificationKit';
import { BusinessError } from '@kit.BasicServicesKit';
import { wantAgent } from '@kit.AbilityKit';
import { Plugin, Invoke } from '@tauri/app';

// ─── Types ───

interface NotificationData {
  id?: number;
  channelId?: string;
  title?: string;
  body?: string;
  schedule?: object;
  largeBody?: string;
  summary?: string;
  actionTypeId?: string;
  group?: string;
  groupSummary?: boolean;
  sound?: string;
  inboxLines?: string[];
  icon?: string;
  largeIcon?: string;
  iconColor?: string;
  attachments?: object[];
  extra?: Record<string, Object>;
  ongoing?: boolean;
  autoCancel?: boolean;
  silent?: boolean;
}

// ─── Action Types (mirrors Rust ActionType) ───

interface ActionDef {
  id: string;
  title: string;
  requiresAuthentication: boolean;
  foreground: boolean;
  destructive: boolean;
  input: boolean;
  inputButtonTitle?: string;
  inputPlaceholder?: string;
}

interface RegisteredActionType {
  id: string;
  actions: ActionDef[];
  hiddenPreviewsBodyPlaceholder?: string;
  customDismissAction: boolean;
  allowInCarPlay: boolean;
  hiddenPreviewsShowTitle: boolean;
  hiddenPreviewsShowSubtitle: boolean;
}

interface RegisterActionTypesArgs {
  types: RegisteredActionType[];
}

// ─── Schedule variants (externally-tagged serde enum) ───

interface ScheduleAt {
  date: string;
  repeating: boolean;
  allowWhileIdle: boolean;
}

interface ScheduleIntervalFields {
  year?: number;
  month?: number;
  day?: number;
  weekday?: number;
  hour?: number;
  minute?: number;
  second?: number;
}

interface ScheduleInterval {
  interval: ScheduleIntervalFields;
  allowWhileIdle: boolean;
}

interface ScheduleEvery {
  interval: string;
  count: number;
  allowWhileIdle: boolean;
}

interface ScheduleWrapper {
  at?: ScheduleAt;
  interval?: ScheduleInterval;
  every?: ScheduleEvery;
}

// ─── Pending notification tracking ───

interface PendingEntry {
  id: number;
  title: string;
  body: string;
  schedule: object;
  timerId: number;
}

interface PendingNotification {
  id: number;
  title: string;
  body: string;
  schedule: object;
}

interface ChannelConfig {
  id: string;
  name: string;
  description?: string;
  importance?: number;
  sound?: string;
  lights?: boolean;
  lightColor?: string;
  vibration?: boolean;
  visibility?: number;
  slotType: notificationManager.SlotType;
}

interface ChannelPayload {
  id: string;
  name: string;
  description?: string;
  importance?: number;
  sound?: string;
  lights?: boolean;
  lightColor?: string;
  vibration?: boolean;
  visibility?: number;
}

interface NotificationIdRef {
  id: number;
}

interface BatchArgs {
  notifications: NotificationData[];
}

interface CancelArgs {
  notifications: number[];
}

interface RemoveActiveArgs {
  notifications: NotificationIdRef[];
}

interface DeleteChannelArgs {
  id: string;
}

// ─── Listener registration (register_listener / remove_listener) ───

// Wire format from JS addPluginListener: { event: string, handler: "__CHANNEL__:123" }
interface RegisterListenerArgs {
  event: string;
  handler: string;
}

// Wire format from JS PluginListener.unregister: { event: string, channelId: number }
interface RemoveListenerArgs {
  event: string;
  channelId: number;
}

// Payload emitted on the "actionPerformed" channel when a notification action
// button is clicked (via WantAgent cold-start or onNewWant warm-start).
interface ActionClickPayload {
  id: number;
  actionId: string;
}

// ─── Importance → SlotType mapping ───

function importanceToSlotType(importance: number | undefined): notificationManager.SlotType {
  switch (importance) {
    case 4: // Importance.High
      return notificationManager.SlotType.SOCIAL_COMMUNICATION;
    case 3: // Importance.Default
      return notificationManager.SlotType.SERVICE_INFORMATION;
    case 2: // Importance.Low
      return notificationManager.SlotType.CONTENT_INFORMATION;
    default: // Importance.Min(1), None(0), undefined
      return notificationManager.SlotType.OTHER_TYPES;
  }
}

// ─── Plugin Implementation ───

export class NotificationPlugin extends Plugin {
  private channelMap: Map<string, ChannelConfig> = new Map();
  // Track SlotType usage to prevent shared slots from being removed prematurely
  private slotTypeRefCount: Map<number, number> = new Map();
  // Registered action types: actionTypeId → definition with actions
  private actionTypeMap: Map<string, RegisteredActionType> = new Map();
  // Pending notifications scheduled via foreground timers
  private pendingMap: Map<number, PendingEntry> = new Map();
  // Registered event listener channels: event name → channel id.
  // Populated by registerListener; used by onNotificationAction to emit.
  private listenerChannels: Map<string, number> = new Map();

  getCommands(): Map<string, (invoke: Invoke) => void> {
    const commands: Map<string, (invoke: Invoke) => void> = new Map();
    commands.set('show', (invoke: Invoke): void => { this.handleShow(invoke); });
    commands.set('batch', (invoke: Invoke): void => { this.handleBatch(invoke); });
    commands.set('cancel', (invoke: Invoke): void => { this.handleCancel(invoke); });
    commands.set('removeActive', (invoke: Invoke): void => { this.handleRemoveActive(invoke); });
    commands.set('getActive', (invoke: Invoke): void => { invoke.resolve(JSON.stringify([])); });
    commands.set('getPending', (invoke: Invoke): void => { this.handleGetPending(invoke); });
    commands.set('requestPermissions', (invoke: Invoke): void => { this.handleRequestPermissions(invoke); });
    commands.set('checkPermissions', (invoke: Invoke): void => { this.handleCheckPermissions(invoke); });
    commands.set('createChannel', (invoke: Invoke): void => { this.handleCreateChannel(invoke); });
    commands.set('deleteChannel', (invoke: Invoke): void => { this.handleDeleteChannel(invoke); });
    commands.set('listChannels', (invoke: Invoke): void => { this.handleListChannels(invoke); });
    commands.set('registerActionTypes', (invoke: Invoke): void => { this.handleRegisterActionTypes(invoke); });
    commands.set('registerListener', (invoke: Invoke): void => { this.handleRegisterListener(invoke); });
    commands.set('removeListener', (invoke: Invoke): void => { this.handleRemoveListener(invoke); });
    return commands;
  }

  // ─── show ───

  private handleShow(invoke: Invoke): void {
    this.publishNotification(invoke);
  }

  private async publishNotification(invoke: Invoke): Promise<void> {
    try {
      const args: NotificationData = JSON.parse(invoke.parseArgs()) as NotificationData;
      const id = args.id ?? 0;

      // If a schedule is present, defer to the foreground timer
      if (args.schedule != null) {
        this.scheduleNotification(id, args, invoke);
        return;
      }

      const request = await this.buildRequest(id, args);
      await notificationManager.publish(request);
      invoke.resolve(JSON.stringify(id));
    } catch (e) {
      const err = e as BusinessError;
      console.error('[NotificationPlugin] publish failed: ' + err.message);
      invoke.reject('Notification publish failed: ' + err.message);
    }
  }

  /**
   * Builds a NotificationRequest from NotificationData, including action buttons
   * if an actionTypeId is registered.
   */
  private async buildRequest(id: number, args: NotificationData): Promise<notificationManager.NotificationRequest> {
    const request: notificationManager.NotificationRequest = {
      id: id,
      content: this.buildContent(args),
      tapDismissed: args.autoCancel !== false
    };

    // channel_id → SlotType lookup
    if (args.channelId != null) {
      const channelConfig = this.channelMap.get(args.channelId);
      if (channelConfig != null) {
        request.notificationSlotType = channelConfig.slotType;
      } else {
        console.warn('[NotificationPlugin] channelId "' + args.channelId + '" not found in local mapping');
      }
    }

    // sound: must be rawfile name or sandbox URI
    if (args.sound != null) {
      request.sound = args.sound;
    }

    // action buttons: look up registered action type and build NotificationActionButtons
    if (args.actionTypeId != null) {
      const buttons = await this.buildActionButtons(args.actionTypeId, id);
      if (buttons != null && buttons.length > 0) {
        request.actionButtons = buttons;
      }
    }

    return request;
  }

  /**
   * Builds action buttons from a registered ActionType.
   * Each button gets a WantAgent that starts the app ability with action parameters.
   *
   * Action button clicks are dispatched via EntryAbility.handleNotificationAction
   * (cold-start in onCreate, warm-start in onNewWant), which calls
   * NotificationPlugin.onNotificationAction, which emits on the
   * "actionPerformed" listener channel registered via registerListener.
   */
  private async buildActionButtons(actionTypeId: string, notificationId: number): Promise<notificationManager.NotificationActionButton[] | null> {
    const actionType = this.actionTypeMap.get(actionTypeId);
    if (actionType == null) {
      console.warn('[NotificationPlugin] actionTypeId "' + actionTypeId + '" not registered');
      return null;
    }
    if (this.context == null) {
      console.warn('[NotificationPlugin] Cannot build action buttons: context is null');
      return null;
    }

    const bundleName = this.context.abilityInfo.bundleName;
    const abilityName = this.context.abilityInfo.name;

    const buttons: notificationManager.NotificationActionButton[] = [];
    for (let i = 0; i < actionType.actions.length; i++) {
      const action = actionType.actions[i];
      const wantAgentInfo: wantAgent.WantAgentInfo = {
        wants: [{
          bundleName: bundleName,
          abilityName: abilityName,
          parameters: {
            _tauriNotificationId: String(notificationId),
            _tauriActionId: action.id,
            _tauriActionTypeId: actionTypeId
          }
        }],
        actionType: wantAgent.OperationType.START_ABILITY,
        requestCode: 0,
        actionFlags: [wantAgent.WantAgentFlags.UPDATE_PRESENT_FLAG]
      };
      const wa = await wantAgent.getWantAgent(wantAgentInfo);
      // NotificationActionButton = { title, wantAgent } — no icon field in the
      // SDK type (notificationActionButton.d.ts); the button renders text-only.
      const button: notificationManager.NotificationActionButton = {
        title: action.title,
        wantAgent: wa
      };
      buttons.push(button);
    }
    return buttons;
  }

  // ─── scheduling (foreground timers) ───

  /**
   * Schedules a notification using foreground setTimeout/setInterval.
   *
   * Limitations:
   * - Only works while the app is in the foreground (timers are cleared on exit)
   * - For background scheduling, use reminderAgentManager.publishReminder (not implemented)
   * - Repeating schedules use setInterval with the initial delay as the repeat period
   */
  private scheduleNotification(id: number, args: NotificationData, invoke: Invoke): void {
    try {
      const schedule = args.schedule as ScheduleWrapper;
      const title = args.title ?? '';
      const body = args.body ?? '';

      let delayMs: number = 0;
      let repeating: boolean = false;

      if (schedule.at != null) {
        const timestamp = Date.parse(schedule.at.date);
        if (isNaN(timestamp)) {
          invoke.reject('Invalid schedule date: ' + schedule.at.date);
          return;
        }
        delayMs = timestamp - Date.now();
        if (delayMs < 0) {
          delayMs = 0;
        }
        repeating = schedule.at.repeating;
      } else if (schedule.interval != null) {
        delayMs = this.intervalToMs(schedule.interval.interval);
        repeating = true; // interval schedules repeat
      } else if (schedule.every != null) {
        delayMs = this.everyToMs(schedule.every.interval, schedule.every.count);
        repeating = true; // every schedules repeat
      } else {
        invoke.reject('Unsupported schedule format');
        return;
      }

      const self = this;
      const publishFn = (): void => {
        self.doScheduledPublish(id, args);
      };

      let timerId: number;
      if (repeating) {
        timerId = setInterval(publishFn, delayMs);
      } else {
        timerId = setTimeout(publishFn, delayMs);
      }

      this.pendingMap.set(id, {
        id: id,
        title: title,
        body: body,
        schedule: args.schedule as object,
        timerId: timerId
      });

      invoke.resolve(JSON.stringify(id));
    } catch (e) {
      const err = e as BusinessError;
      console.error('[NotificationPlugin] schedule failed: ' + err.message);
      invoke.reject('Schedule failed: ' + err.message);
    }
  }

  private async doScheduledPublish(id: number, args: NotificationData): Promise<void> {
    try {
      const request = await this.buildRequest(id, args);
      await notificationManager.publish(request);
    } catch (e) {
      const err = e as BusinessError;
      console.error('[NotificationPlugin] scheduled publish failed: ' + err.message);
    }
  }

  /** Converts a ScheduleInterval to milliseconds (approximate). */
  private intervalToMs(interval: ScheduleIntervalFields): number {
    let ms: number = 0;
    if (interval.year != null) ms += interval.year * 365 * 24 * 60 * 60 * 1000;
    if (interval.month != null) ms += interval.month * 30 * 24 * 60 * 60 * 1000;
    if (interval.day != null) ms += interval.day * 24 * 60 * 60 * 1000;
    if (interval.hour != null) ms += interval.hour * 60 * 60 * 1000;
    if (interval.minute != null) ms += interval.minute * 60 * 1000;
    if (interval.second != null) ms += interval.second * 1000;
    if (ms <= 0) {
      ms = 60 * 1000; // default 1 minute
    }
    return ms;
  }

  /** Converts a ScheduleEvery (interval string + count) to milliseconds. */
  private everyToMs(interval: string, count: number): number {
    let unitMs: number;
    switch (interval) {
      case 'year': unitMs = 365 * 24 * 60 * 60 * 1000; break;
      case 'month': unitMs = 30 * 24 * 60 * 60 * 1000; break;
      case 'twoWeeks': unitMs = 14 * 24 * 60 * 60 * 1000; break;
      case 'week': unitMs = 7 * 24 * 60 * 60 * 1000; break;
      case 'day': unitMs = 24 * 60 * 60 * 1000; break;
      case 'hour': unitMs = 60 * 60 * 1000; break;
      case 'minute': unitMs = 60 * 1000; break;
      case 'second': unitMs = 1000; break;
      default: unitMs = 60 * 1000; break;
    }
    return unitMs * count;
  }

  private buildContent(args: NotificationData): notificationManager.NotificationContent {
    const title = args.title ?? '';
    const text = args.body ?? '';
    const additionalText = args.summary ?? '';

    // long text style when largeBody is present
    if (args.largeBody != null && args.largeBody.length > 0) {
      const longContent: notificationManager.NotificationLongTextContent = {
        title: title,
        text: text,
        additionalText: additionalText,
        longText: args.largeBody,
        briefText: text,
        expandedTitle: title
      };
      const content: notificationManager.NotificationContent = {
        notificationContentType: notificationManager.ContentType.NOTIFICATION_CONTENT_LONG_TEXT,
        longText: longContent
      };
      return content;
    }

    // multiline style when inboxLines present
    if (args.inboxLines != null && args.inboxLines.length > 0) {
      const multiContent: notificationManager.NotificationMultiLineContent = {
        title: title,
        text: text,
        additionalText: additionalText,
        lines: args.inboxLines,
        briefText: args.summary ?? '',
        longTitle: title
      };
      const content: notificationManager.NotificationContent = {
        notificationContentType: notificationManager.ContentType.NOTIFICATION_CONTENT_MULTILINE,
        multiLine: multiContent
      };
      return content;
    }

    // default: basic text
    const basicContent: notificationManager.NotificationBasicContent = {
      title: title,
      text: text,
      additionalText: additionalText
    };
    const content: notificationManager.NotificationContent = {
      notificationContentType: notificationManager.ContentType.NOTIFICATION_CONTENT_BASIC_TEXT,
      normal: basicContent
    };
    return content;
  }

  // ─── batch ───

  private handleBatch(invoke: Invoke): void {
    this.publishBatch(invoke);
  }

  private async publishBatch(invoke: Invoke): Promise<void> {
    try {
      const args: BatchArgs = JSON.parse(invoke.parseArgs()) as BatchArgs;
      const notifications: NotificationData[] = Array.isArray(args) ? args as NotificationData[] : args.notifications;
      const ids: number[] = [];

      // Start auto-IDs at 10000 to avoid conflicts with user-defined IDs (typically small numbers)
      let nextAutoId: number = 10000;

      for (const data of notifications) {
        const id = data.id ?? nextAutoId++;
        const request = await this.buildRequest(id, data);
        await notificationManager.publish(request);
        ids.push(id);
      }

      invoke.resolve(JSON.stringify(ids));
    } catch (e) {
      const err = e as BusinessError;
      console.error('[NotificationPlugin] batch failed: ' + err.message);
      invoke.reject('Batch publish failed: ' + err.message);
    }
  }

  // ─── cancel ───

  private handleCancel(invoke: Invoke): void {
    this.cancelNotifications(invoke);
  }

  private async cancelNotifications(invoke: Invoke): Promise<void> {
    try {
      const argsStr = invoke.parseArgs();

      if (argsStr === '' || argsStr === 'null' || argsStr === '{}') {
        // cancelAll: clear all pending timers + system notifications
        this.clearAllPending();
        await notificationManager.cancelAll();
        invoke.resolve('');
        return;
      }

      const args: CancelArgs = JSON.parse(argsStr) as CancelArgs;
      const notifications: number[] = args.notifications;

      if (notifications != null && notifications.length > 0) {
        for (const id of notifications) {
          this.clearPending(id);
          try {
            await notificationManager.cancel(id);
          } catch (cancelErr) {
            // Silently ignore if notification doesn't exist (OHOS throws, Android doesn't)
            console.warn('[NotificationPlugin] cancel id=' + id + ' failed (may not exist): ' + (cancelErr as BusinessError).message);
          }
        }
      } else {
        this.clearAllPending();
        await notificationManager.cancelAll();
      }

      invoke.resolve('');
    } catch (e) {
      const err = e as BusinessError;
      console.error('[NotificationPlugin] cancel failed: ' + err.message);
      invoke.reject('Cancel failed: ' + err.message);
    }
  }

  /** Clears a single pending timer by notification ID. */
  private clearPending(id: number): void {
    const entry = this.pendingMap.get(id);
    if (entry != null) {
      clearTimeout(entry.timerId);
      this.pendingMap.delete(id);
    }
  }

  /** Clears all pending timers. */
  private clearAllPending(): void {
    const keys: number[] = [];
    for (const key of this.pendingMap.keys()) {
      keys.push(key);
    }
    for (let i = 0; i < keys.length; i++) {
      this.clearPending(keys[i]);
    }
  }

  // ─── registerActionTypes ───

  private handleRegisterActionTypes(invoke: Invoke): void {
    this.registerActionTypes(invoke);
  }

  private async registerActionTypes(invoke: Invoke): Promise<void> {
    try {
      const args: RegisterActionTypesArgs = JSON.parse(invoke.parseArgs()) as RegisterActionTypesArgs;
      const types: RegisteredActionType[] = args.types;
      if (types != null) {
        for (let i = 0; i < types.length; i++) {
          const t = types[i];
          this.actionTypeMap.set(t.id, t);
        }
      }
      invoke.resolve('');
    } catch (e) {
      const err = e as BusinessError;
      console.error('[NotificationPlugin] registerActionTypes failed: ' + err.message);
      invoke.reject('registerActionTypes failed: ' + err.message);
    }
  }

  // ─── registerListener ───

  private handleRegisterListener(invoke: Invoke): void {
    try {
      const args: RegisterListenerArgs = JSON.parse(invoke.parseArgs()) as RegisterListenerArgs;
      const channelId = Plugin.parseChannelId(args.handler);
      if (channelId == null) {
        invoke.reject('registerListener: invalid channel handler: ' + args.handler);
        return;
      }
      this.listenerChannels.set(args.event, channelId);
      console.info('[NotificationPlugin] Listener registered for event: ' + args.event + ' (channel ' + channelId + ')');
      // Note: "actionPerformed" events originate from EntryAbility.handleNotificationAction
      // (WantAgent cold-start / onNewWant warm-start). "notification" events
      // (onNotificationReceived) have no corresponding OHOS subscription API;
      // registration succeeds but no events will be delivered.
      invoke.resolve('');
    } catch (e) {
      const err = e as BusinessError;
      console.error('[NotificationPlugin] registerListener failed: ' + err.message);
      invoke.reject('registerListener failed: ' + err.message);
    }
  }

  // ─── removeListener ───

  private handleRemoveListener(invoke: Invoke): void {
    try {
      const args: RemoveListenerArgs = JSON.parse(invoke.parseArgs()) as RemoveListenerArgs;
      this.listenerChannels.delete(args.event);
      console.info('[NotificationPlugin] Listener removed for event: ' + args.event);
      invoke.resolve('');
    } catch (e) {
      const err = e as BusinessError;
      console.error('[NotificationPlugin] removeListener failed: ' + err.message);
      invoke.reject('removeListener failed: ' + err.message);
    }
  }

  // ─── notification action dispatch (called by EntryAbility) ───

  /**
   * Called by EntryAbility.handleNotificationAction when a notification action
   * button is clicked (via WantAgent). Emits the action click payload on the
   * "actionPerformed" listener channel.
   */
  onNotificationAction(notificationId: number, actionId: string, actionTypeId: string): void {
    const channelId = this.listenerChannels.get('actionPerformed');
    if (channelId != null) {
      const payload: ActionClickPayload = {
        id: notificationId,
        actionId: actionId
      };
      this.emit(channelId, payload);
    } else {
      console.warn('[NotificationPlugin] No listener registered for actionPerformed');
    }
  }

  // ─── getPending ───

  private handleGetPending(invoke: Invoke): void {
    try {
      const pending: PendingNotification[] = [];
      const values: PendingEntry[] = [];
      for (const entry of this.pendingMap.values()) {
        values.push(entry);
      }
      for (let i = 0; i < values.length; i++) {
        const e = values[i];
        pending.push({
          id: e.id,
          title: e.title,
          body: e.body,
          schedule: e.schedule
        });
      }
      invoke.resolve(JSON.stringify(pending));
    } catch (e) {
      const err = e as BusinessError;
      console.error('[NotificationPlugin] getPending failed: ' + err.message);
      invoke.resolve(JSON.stringify([]));
    }
  }

  // ─── removeActive ───

  private handleRemoveActive(invoke: Invoke): void {
    this.removeActive(invoke);
  }

  private async removeActive(invoke: Invoke): Promise<void> {
    try {
      const argsStr = invoke.parseArgs();

      if (argsStr === '' || argsStr === 'null' || argsStr === '{}') {
        await notificationManager.cancelAll();
        invoke.resolve('');
        return;
      }

      const args: RemoveActiveArgs = JSON.parse(argsStr) as RemoveActiveArgs;
      const notifications: NotificationIdRef[] = args.notifications;

      if (notifications != null && notifications.length > 0) {
        for (const n of notifications) {
          try {
            await notificationManager.cancel(n.id);
          } catch (cancelErr) {
            // Silently ignore if notification doesn't exist
            console.warn('[NotificationPlugin] removeActive id=' + n.id + ' failed (may not exist): ' + (cancelErr as BusinessError).message);
          }
        }
      } else {
        await notificationManager.cancelAll();
      }

      invoke.resolve('');
    } catch (e) {
      const err = e as BusinessError;
      console.error('[NotificationPlugin] removeActive failed: ' + err.message);
      invoke.reject('Remove active failed: ' + err.message);
    }
  }

  // ─── requestPermissions ───

  private handleRequestPermissions(invoke: Invoke): void {
    this.requestPermissions(invoke);
  }

  private async requestPermissions(invoke: Invoke): Promise<void> {
    try {
      // Check current state first
      const isEnabled = await notificationManager.isNotificationEnabled();
      if (isEnabled) {
        invoke.resolve(JSON.stringify({ permissionState: 'granted' }));
        return;
      }

      // Request permission — requires UIAbilityContext from Plugin base class
      if (this.context != null) {
        await notificationManager.requestEnableNotification(this.context);
      } else {
        console.warn('[NotificationPlugin] UIAbilityContext not available. User must enable notifications in system settings.');
        invoke.resolve(JSON.stringify({ permissionState: 'denied' }));
        return;
      }
      // Re-check actual state after request (user may have denied)
      const nowEnabled = await notificationManager.isNotificationEnabled();
      const state = nowEnabled ? 'granted' : 'denied';
      invoke.resolve(JSON.stringify({ permissionState: state }));
    } catch (e) {
      const err = e as BusinessError;
      if (err.code === 1600004) {
        // User previously denied — cannot re-prompt
        console.warn('[NotificationPlugin] Notification permission previously denied. User must enable in system settings.');
        invoke.resolve(JSON.stringify({ permissionState: 'denied' }));
      } else {
        console.error('[NotificationPlugin] requestPermissions failed: code=' + err.code + ', message=' + err.message);
        invoke.reject('requestPermissions failed: code=' + err.code + ', message=' + err.message);
      }
    }
  }

  // ─── checkPermissions ───

  private handleCheckPermissions(invoke: Invoke): void {
    this.checkPermissions(invoke);
  }

  private async checkPermissions(invoke: Invoke): Promise<void> {
    try {
      const isEnabled = await notificationManager.isNotificationEnabled();
      const state = isEnabled ? 'granted' : 'denied';
      invoke.resolve(JSON.stringify({ permissionState: state }));
    } catch (e) {
      const err = e as BusinessError;
      console.error('[NotificationPlugin] checkPermissions failed: ' + err.message);
      invoke.resolve(JSON.stringify({ permissionState: 'denied' }));
    }
  }

  // ─── createChannel ───

  private handleCreateChannel(invoke: Invoke): void {
    this.createChannel(invoke);
  }

  private async createChannel(invoke: Invoke): Promise<void> {
    try {
      const args: ChannelPayload = JSON.parse(invoke.parseArgs()) as ChannelPayload;
      const slotType = importanceToSlotType(args.importance);

      // If overwriting existing channel with different SlotType, decrement old refCount
      const oldConfig = this.channelMap.get(args.id);
      if (oldConfig != null && oldConfig.slotType !== slotType) {
        const oldCount = this.slotTypeRefCount.get(oldConfig.slotType) ?? 1;
        if (oldCount <= 1) {
          this.slotTypeRefCount.delete(oldConfig.slotType);
        } else {
          this.slotTypeRefCount.set(oldConfig.slotType, oldCount - 1);
        }
      }

      // Track SlotType usage — warn if shared with another channel
      const existingCount = this.slotTypeRefCount.get(slotType) ?? 0;
      if (existingCount > 0) {
        console.warn('[NotificationPlugin] SlotType ' + slotType + ' already used by ' + existingCount + ' channel(s). ' +
          'OHOS only has 4 SlotType values; channels sharing the same SlotType will overwrite each other\'s system config.');
      }
      this.slotTypeRefCount.set(slotType, existingCount + 1);

      // Create the slot in the system (uses default config)
      await notificationManager.addSlot(slotType);

      // Store full config in local mapping table
      const config: ChannelConfig = {
        id: args.id,
        name: args.name,
        description: args.description,
        importance: args.importance,
        sound: args.sound,
        lights: args.lights,
        lightColor: args.lightColor,
        vibration: args.vibration,
        visibility: args.visibility,
        slotType: slotType
      };
      this.channelMap.set(args.id, config);

      invoke.resolve('');
    } catch (e) {
      const err = e as BusinessError;
      console.error('[NotificationPlugin] createChannel failed: ' + err.message);
      invoke.reject('Create channel failed: ' + err.message);
    }
  }

  // ─── deleteChannel ───

  private handleDeleteChannel(invoke: Invoke): void {
    this.deleteChannel(invoke);
  }

  private async deleteChannel(invoke: Invoke): Promise<void> {
    try {
      const args: DeleteChannelArgs = JSON.parse(invoke.parseArgs()) as DeleteChannelArgs;

      const config = this.channelMap.get(args.id);
      if (config != null) {
        // Only remove system slot when no other channels share this SlotType
        const count = this.slotTypeRefCount.get(config.slotType) ?? 1;
        if (count <= 1) {
          await notificationManager.removeSlot(config.slotType);
          this.slotTypeRefCount.delete(config.slotType);
        } else {
          this.slotTypeRefCount.set(config.slotType, count - 1);
          console.warn('[NotificationPlugin] SlotType ' + config.slotType + ' still used by ' + (count - 1) + ' other channel(s), keeping system slot.');
        }
        this.channelMap.delete(args.id);
      }
      // Idempotent: no error if channel doesn't exist

      invoke.resolve('');
    } catch (e) {
      const err = e as BusinessError;
      console.error('[NotificationPlugin] deleteChannel failed: ' + err.message);
      invoke.reject('Delete channel failed: ' + err.message);
    }
  }

  // ─── listChannels ───

  private handleListChannels(invoke: Invoke): void {
    this.listChannels(invoke);
  }

  private async listChannels(invoke: Invoke): Promise<void> {
    try {
      const systemSlots = await notificationManager.getSlots();
      const channels: ChannelPayload[] = [];

      // Collect local configs into an array for iteration (avoid Map.forEach type issues in ArkTS)
      const localConfigs: ChannelConfig[] = [];
      const configKeys: string[] = [];
      for (const key of this.channelMap.keys()) {
        configKeys.push(key);
      }
      for (let i = 0; i < configKeys.length; i++) {
        const val = this.channelMap.get(configKeys[i]);
        if (val != null) {
          localConfigs.push(val);
        }
      }

      // Merge system slots with local mapping
      for (const slot of systemSlots) {
        // Find local config by matching slotType
        let matchedConfig: ChannelConfig | undefined = undefined;
        for (let j = 0; j < localConfigs.length; j++) {
          if (localConfigs[j].slotType === slot.notificationType) {
            matchedConfig = localConfigs[j];
            break;
          }
        }

        const ch: ChannelPayload = {
          id: matchedConfig != null ? matchedConfig.id : ('slot_' + slot.notificationType),
          name: matchedConfig != null ? matchedConfig.name : ('Channel ' + slot.notificationType),
        };
        // Always set importance to avoid undefined fields in JSON (serde deserialization requires it)
        ch.importance = matchedConfig != null && matchedConfig.importance != null ? matchedConfig.importance : 3; // Default importance
        if (matchedConfig != null) {
          ch.description = matchedConfig.description;
          ch.sound = matchedConfig.sound;
          ch.lights = matchedConfig.lights;
          ch.lightColor = matchedConfig.lightColor;
          ch.vibration = matchedConfig.vibration;
          ch.visibility = matchedConfig.visibility;
        }
        channels.push(ch);
      }

      // Include local-only channels that don't have a system slot yet
      for (let k = 0; k < localConfigs.length; k++) {
        const config = localConfigs[k];
        let found = false;
        for (let m = 0; m < channels.length; m++) {
          if (channels[m].id === config.id) {
            found = true;
            break;
          }
        }
        if (!found) {
          const localCh: ChannelPayload = {
            id: config.id,
            name: config.name,
          };
          localCh.importance = config.importance != null ? config.importance : 3;
          localCh.description = config.description;
          localCh.sound = config.sound;
          localCh.lights = config.lights;
          localCh.lightColor = config.lightColor;
          localCh.vibration = config.vibration;
          localCh.visibility = config.visibility;
          channels.push(localCh);
        }
      }

      invoke.resolve(JSON.stringify(channels));
    } catch (e) {
      const err = e as BusinessError;
      console.error('[NotificationPlugin] listChannels failed: ' + err.message);
      invoke.reject('List channels failed: ' + err.message);
    }
  }
}

export default NotificationPlugin;
