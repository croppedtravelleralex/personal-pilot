export namespace backend {

	export class AutomationRuleInfo {
	    id: string;
	    name: string;
	    triggerEvent: string;
	    condition?: string;
	    action: string;
	    actionParams?: Record<string, any>;
	    cooldown: string;
	    enabled: boolean;
	    createdAt: string;
	    updatedAt: string;

	    static createFrom(source: any = {}) {
	        return new AutomationRuleInfo(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.id = source["id"];
	        this.name = source["name"];
	        this.triggerEvent = source["triggerEvent"];
	        this.condition = source["condition"];
	        this.action = source["action"];
	        this.actionParams = source["actionParams"];
	        this.cooldown = source["cooldown"];
	        this.enabled = source["enabled"];
	        this.createdAt = source["createdAt"];
	        this.updatedAt = source["updatedAt"];
	    }
	}
	export class AutomationRuleInput {
	    name: string;
	    triggerEvent: string;
	    condition?: string;
	    action: string;
	    actionParams?: Record<string, any>;
	    cooldown: string;
	    enabled: boolean;

	    static createFrom(source: any = {}) {
	        return new AutomationRuleInput(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.name = source["name"];
	        this.triggerEvent = source["triggerEvent"];
	        this.condition = source["condition"];
	        this.action = source["action"];
	        this.actionParams = source["actionParams"];
	        this.cooldown = source["cooldown"];
	        this.enabled = source["enabled"];
	    }
	}
	export class BehaviorPresetInfo {
	    id: string;
	    name: string;
	    description: string;

	    static createFrom(source: any = {}) {
	        return new BehaviorPresetInfo(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.id = source["id"];
	        this.name = source["name"];
	        this.description = source["description"];
	    }
	}
	export class CookieInfo {
	    name: string;
	    value: string;
	    domain: string;
	    path: string;
	    expires: number;
	    httpOnly: boolean;
	    secure: boolean;
	    sameSite: string;

	    static createFrom(source: any = {}) {
	        return new CookieInfo(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.name = source["name"];
	        this.value = source["value"];
	        this.domain = source["domain"];
	        this.path = source["path"];
	        this.expires = source["expires"];
	        this.httpOnly = source["httpOnly"];
	        this.secure = source["secure"];
	        this.sameSite = source["sameSite"];
	    }
	}
	export class EventLogQueryInput {
	    after: string;
	    before: string;
	    namespace: string;
	    severity: string;
	    eventName: string;
	    limit: number;
	    offset: number;

	    static createFrom(source: any = {}) {
	        return new EventLogQueryInput(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.after = source["after"];
	        this.before = source["before"];
	        this.namespace = source["namespace"];
	        this.severity = source["severity"];
	        this.eventName = source["eventName"];
	        this.limit = source["limit"];
	        this.offset = source["offset"];
	    }
	}
	export class LicenseStatus {
	    maxLimit: number;
	    usedCount: number;
	    usedKeys: string[];

	    static createFrom(source: any = {}) {
	        return new LicenseStatus(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.maxLimit = source["maxLimit"];
	        this.usedCount = source["usedCount"];
	        this.usedKeys = source["usedKeys"];
	    }
	}
	export class ProxyIPHealthResult {
	    proxyId: string;
	    ok: boolean;
	    source: string;
	    error: string;
	    ip: string;
	    fraudScore: number;
	    isResidential: boolean;
	    isBroadcast: boolean;
	    country: string;
	    region: string;
	    city: string;
	    asOrganization: string;
	    rawData: Record<string, any>;
	    updatedAt: string;

	    static createFrom(source: any = {}) {
	        return new ProxyIPHealthResult(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.proxyId = source["proxyId"];
	        this.ok = source["ok"];
	        this.source = source["source"];
	        this.error = source["error"];
	        this.ip = source["ip"];
	        this.fraudScore = source["fraudScore"];
	        this.isResidential = source["isResidential"];
	        this.isBroadcast = source["isBroadcast"];
	        this.country = source["country"];
	        this.region = source["region"];
	        this.city = source["city"];
	        this.asOrganization = source["asOrganization"];
	        this.rawData = source["rawData"];
	        this.updatedAt = source["updatedAt"];
	    }
	}
	export class ProxyTestResult {
	    proxyId: string;
	    ok: boolean;
	    latencyMs: number;
	    error: string;

	    static createFrom(source: any = {}) {
	        return new ProxyTestResult(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.proxyId = source["proxyId"];
	        this.ok = source["ok"];
	        this.latencyMs = source["latencyMs"];
	        this.error = source["error"];
	    }
	}
	export class ProxyValidationResult {
	    supported: boolean;
	    errorMsg: string;

	    static createFrom(source: any = {}) {
	        return new ProxyValidationResult(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.supported = source["supported"];
	        this.errorMsg = source["errorMsg"];
	    }
	}
	export class SchedulerTaskAction {
	    type: string;
	    target: string;
	    value: string;
	    timeout: number;

	    static createFrom(source: any = {}) {
	        return new SchedulerTaskAction(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.type = source["type"];
	        this.target = source["target"];
	        this.value = source["value"];
	        this.timeout = source["timeout"];
	    }
	}
	export class SchedulerTaskTrigger {
	    type: string;
	    cron?: string;
	    interval?: string;
	    event?: string;

	    static createFrom(source: any = {}) {
	        return new SchedulerTaskTrigger(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.type = source["type"];
	        this.cron = source["cron"];
	        this.interval = source["interval"];
	        this.event = source["event"];
	    }
	}
	export class SchedulerTaskInfo {
	    id: string;
	    name: string;
	    trigger: SchedulerTaskTrigger;
	    actions: SchedulerTaskAction[];
	    maxRetries: number;
	    retryDelay: string;
	    dependsOn: string[];
	    profileId: string;
	    enabled: boolean;
	    createdAt: string;
	    status: string;
	    lastRunAt: string;
	    lastError: string;
	    retryCount: number;

	    static createFrom(source: any = {}) {
	        return new SchedulerTaskInfo(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.id = source["id"];
	        this.name = source["name"];
	        this.trigger = this.convertValues(source["trigger"], SchedulerTaskTrigger);
	        this.actions = this.convertValues(source["actions"], SchedulerTaskAction);
	        this.maxRetries = source["maxRetries"];
	        this.retryDelay = source["retryDelay"];
	        this.dependsOn = source["dependsOn"];
	        this.profileId = source["profileId"];
	        this.enabled = source["enabled"];
	        this.createdAt = source["createdAt"];
	        this.status = source["status"];
	        this.lastRunAt = source["lastRunAt"];
	        this.lastError = source["lastError"];
	        this.retryCount = source["retryCount"];
	    }

		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	export class SchedulerTaskInput {
	    name: string;
	    trigger: SchedulerTaskTrigger;
	    actions: SchedulerTaskAction[];
	    maxRetries: number;
	    retryDelay: string;
	    dependsOn: string[];
	    profileId: string;
	    enabled: boolean;

	    static createFrom(source: any = {}) {
	        return new SchedulerTaskInput(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.name = source["name"];
	        this.trigger = this.convertValues(source["trigger"], SchedulerTaskTrigger);
	        this.actions = this.convertValues(source["actions"], SchedulerTaskAction);
	        this.maxRetries = source["maxRetries"];
	        this.retryDelay = source["retryDelay"];
	        this.dependsOn = source["dependsOn"];
	        this.profileId = source["profileId"];
	        this.enabled = source["enabled"];
	    }

		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}

	export class SnapshotInfo {
	    snapshotId: string;
	    profileId: string;
	    name: string;
	    sizeMB: number;
	    createdAt: string;
	    filePath?: string;

	    static createFrom(source: any = {}) {
	        return new SnapshotInfo(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.snapshotId = source["snapshotId"];
	        this.profileId = source["profileId"];
	        this.name = source["name"];
	        this.sizeMB = source["sizeMB"];
	        this.createdAt = source["createdAt"];
	        this.filePath = source["filePath"];
	    }
	}
	export class SyncWindow {
	    profileId: string;
	    profileName: string;
	    url: string;
	    title: string;
	    debugPort: number;
	    pid: number;
	    status: string;
	    groupId: string;

	    static createFrom(source: any = {}) {
	        return new SyncWindow(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.profileId = source["profileId"];
	        this.profileName = source["profileName"];
	        this.url = source["url"];
	        this.title = source["title"];
	        this.debugPort = source["debugPort"];
	        this.pid = source["pid"];
	        this.status = source["status"];
	        this.groupId = source["groupId"];
	    }
	}
	export class SyncGroup {
	    id: string;
	    name: string;
	    windows: SyncWindow[];

	    static createFrom(source: any = {}) {
	        return new SyncGroup(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.id = source["id"];
	        this.name = source["name"];
	        this.windows = this.convertValues(source["windows"], SyncWindow);
	    }

		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	export class SyncOperation {
	    id: string;
	    type: string;
	    groupId: string;
	    payload?: Record<string, any>;
	    timestamp: string;
	    status: string;
	    error?: string;

	    static createFrom(source: any = {}) {
	        return new SyncOperation(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.id = source["id"];
	        this.type = source["type"];
	        this.groupId = source["groupId"];
	        this.payload = source["payload"];
	        this.timestamp = source["timestamp"];
	        this.status = source["status"];
	        this.error = source["error"];
	    }
	}

	export class SyncWindowPlacement {
	    profileId: string;
	    profileName: string;
	    pid: number;
	    found: boolean;
	    x: number;
	    y: number;
	    width: number;
	    height: number;
	    error?: string;

	    static createFrom(source: any = {}) {
	        return new SyncWindowPlacement(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.profileId = source["profileId"];
	        this.profileName = source["profileName"];
	        this.pid = source["pid"];
	        this.found = source["found"];
	        this.x = source["x"];
	        this.y = source["y"];
	        this.width = source["width"];
	        this.height = source["height"];
	        this.error = source["error"];
	    }
	}
	export class WorkbenchTask {
	    id: string;
	    type: string;
	    profileId: string;
	    profileName: string;
	    detail: string;
	    status: string;
	    createdAt: string;
	    updatedAt: string;
	    error?: string;

	    static createFrom(source: any = {}) {
	        return new WorkbenchTask(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.id = source["id"];
	        this.type = source["type"];
	        this.profileId = source["profileId"];
	        this.profileName = source["profileName"];
	        this.detail = source["detail"];
	        this.status = source["status"];
	        this.createdAt = source["createdAt"];
	        this.updatedAt = source["updatedAt"];
	        this.error = source["error"];
	    }
	}

}

export namespace backup {

	export class ManifestEntry {
	    id: string;
	    category: string;
	    entryType: string;
	    required: boolean;
	    archivePath: string;
	    description?: string;

	    static createFrom(source: any = {}) {
	        return new ManifestEntry(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.id = source["id"];
	        this.category = source["category"];
	        this.entryType = source["entryType"];
	        this.required = source["required"];
	        this.archivePath = source["archivePath"];
	        this.description = source["description"];
	    }
	}
	export class ManifestAppInfo {
	    name: string;
	    version: string;

	    static createFrom(source: any = {}) {
	        return new ManifestAppInfo(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.name = source["name"];
	        this.version = source["version"];
	    }
	}
	export class Manifest {
	    format: string;
	    manifestVersion: number;
	    createdAt: string;
	    app: ManifestAppInfo;
	    entries: ManifestEntry[];

	    static createFrom(source: any = {}) {
	        return new Manifest(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.format = source["format"];
	        this.manifestVersion = source["manifestVersion"];
	        this.createdAt = source["createdAt"];
	        this.app = this.convertValues(source["app"], ManifestAppInfo);
	        this.entries = this.convertValues(source["entries"], ManifestEntry);
	    }

		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}


	export class ScopeEntry {
	    id: string;
	    category: string;
	    entryType: string;
	    required: boolean;
	    sourcePath: string;
	    archivePath: string;
	    exists: boolean;
	    description?: string;

	    static createFrom(source: any = {}) {
	        return new ScopeEntry(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.id = source["id"];
	        this.category = source["category"];
	        this.entryType = source["entryType"];
	        this.required = source["required"];
	        this.sourcePath = source["sourcePath"];
	        this.archivePath = source["archivePath"];
	        this.exists = source["exists"];
	        this.description = source["description"];
	    }
	}
	export class Scope {
	    format: string;
	    manifestVersion: number;
	    appRoot: string;
	    entries: ScopeEntry[];

	    static createFrom(source: any = {}) {
	        return new Scope(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.format = source["format"];
	        this.manifestVersion = source["manifestVersion"];
	        this.appRoot = source["appRoot"];
	        this.entries = this.convertValues(source["entries"], ScopeEntry);
	    }

		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}

}

export namespace behavior {

	export class ActiveRecordingStatus {
	    active: boolean;
	    count: number;
	    profileId?: string;
	    profileIds: string[];
	    inMemoryProfileIds?: string[];
	    recoverableProfileIds?: string[];

	    static createFrom(source: any = {}) {
	        return new ActiveRecordingStatus(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.active = source["active"];
	        this.count = source["count"];
	        this.profileId = source["profileId"];
	        this.profileIds = source["profileIds"];
	        this.inMemoryProfileIds = source["inMemoryProfileIds"];
	        this.recoverableProfileIds = source["recoverableProfileIds"];
	    }
	}
	export class IdleProfile {
	    mouseWander: boolean;
	    tabSwitch: boolean;
	    focusLoss: boolean;

	    static createFrom(source: any = {}) {
	        return new IdleProfile(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.mouseWander = source["mouseWander"];
	        this.tabSwitch = source["tabSwitch"];
	        this.focusLoss = source["focusLoss"];
	    }
	}
	export class KeyboardProfile {
	    enabled: boolean;
	    baseDelayMs: number;
	    delayStdDevMs: number;
	    burstProb: number;
	    burstKeys: number;
	    typoProb: number;
	    bigramDelays: boolean;

	    static createFrom(source: any = {}) {
	        return new KeyboardProfile(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.enabled = source["enabled"];
	        this.baseDelayMs = source["baseDelayMs"];
	        this.delayStdDevMs = source["delayStdDevMs"];
	        this.burstProb = source["burstProb"];
	        this.burstKeys = source["burstKeys"];
	        this.typoProb = source["typoProb"];
	        this.bigramDelays = source["bigramDelays"];
	    }
	}
	export class MouseProfile {
	    enabled: boolean;
	    idleMoveInterval: string;
	    curveStyle: string;
	    speedMean: number;
	    speedStdDev: number;
	    jitterPx: number;
	    pauseProb: number;
	    pauseMaxMs: number;

	    static createFrom(source: any = {}) {
	        return new MouseProfile(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.enabled = source["enabled"];
	        this.idleMoveInterval = source["idleMoveInterval"];
	        this.curveStyle = source["curveStyle"];
	        this.speedMean = source["speedMean"];
	        this.speedStdDev = source["speedStdDev"];
	        this.jitterPx = source["jitterPx"];
	        this.pauseProb = source["pauseProb"];
	        this.pauseMaxMs = source["pauseMaxMs"];
	    }
	}
	export class ScrollProfile {
	    enabled: boolean;
	    idleScrollProb: number;
	    scrollStepPx: number;
	    scrollStepStdDev: number;
	    pauseBetweenMs: number;
	    pauseStdDevMs: number;
	    overscrollProb: number;
	    reverseProb: number;

	    static createFrom(source: any = {}) {
	        return new ScrollProfile(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.enabled = source["enabled"];
	        this.idleScrollProb = source["idleScrollProb"];
	        this.scrollStepPx = source["scrollStepPx"];
	        this.scrollStepStdDev = source["scrollStepStdDev"];
	        this.pauseBetweenMs = source["pauseBetweenMs"];
	        this.pauseStdDevMs = source["pauseStdDevMs"];
	        this.overscrollProb = source["overscrollProb"];
	        this.reverseProb = source["reverseProb"];
	    }
	}
	export class Profile {
	    id: string;
	    name: string;
	    description: string;
	    mouse: MouseProfile;
	    keyboard: KeyboardProfile;
	    scroll: ScrollProfile;
	    idle: IdleProfile;

	    static createFrom(source: any = {}) {
	        return new Profile(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.id = source["id"];
	        this.name = source["name"];
	        this.description = source["description"];
	        this.mouse = this.convertValues(source["mouse"], MouseProfile);
	        this.keyboard = this.convertValues(source["keyboard"], KeyboardProfile);
	        this.scroll = this.convertValues(source["scroll"], ScrollProfile);
	        this.idle = this.convertValues(source["idle"], IdleProfile);
	    }

		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	export class RecordedEvent {
	    t: number;
	    type: string;
	    x?: number;
	    y?: number;
	    btn?: number;
	    key?: string;
	    text?: string;
	    dx?: number;
	    dy?: number;
	    inputType?: string;
	    targetPath?: string;
	    sensitive?: boolean;

	    static createFrom(source: any = {}) {
	        return new RecordedEvent(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.t = source["t"];
	        this.type = source["type"];
	        this.x = source["x"];
	        this.y = source["y"];
	        this.btn = source["btn"];
	        this.key = source["key"];
	        this.text = source["text"];
	        this.dx = source["dx"];
	        this.dy = source["dy"];
	        this.inputType = source["inputType"];
	        this.targetPath = source["targetPath"];
	        this.sensitive = source["sensitive"];
	    }
	}
	export class Recording {
	    id: string;
	    name: string;
	    description: string;
	    events: RecordedEvent[];
	    eventCount?: number;
	    durationMs: number;
	    viewportW: number;
	    viewportH: number;
	    startUrl?: string;
	    currentUrl?: string;
	    title?: string;
	    devicePixelRatio?: number;
	    scale?: number;
	    createdAt: string;

	    static createFrom(source: any = {}) {
	        return new Recording(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.id = source["id"];
	        this.name = source["name"];
	        this.description = source["description"];
	        this.events = this.convertValues(source["events"], RecordedEvent);
	        this.eventCount = source["eventCount"];
	        this.durationMs = source["durationMs"];
	        this.viewportW = source["viewportW"];
	        this.viewportH = source["viewportH"];
	        this.startUrl = source["startUrl"];
	        this.currentUrl = source["currentUrl"];
	        this.title = source["title"];
	        this.devicePixelRatio = source["devicePixelRatio"];
	        this.scale = source["scale"];
	        this.createdAt = source["createdAt"];
	    }

		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	export class RecordingExportBundle {
	    schemaVersion?: string;
	    exportedAt?: string;
	    recording?: Recording;
	    recordings?: Recording[];

	    static createFrom(source: any = {}) {
	        return new RecordingExportBundle(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        Object.assign(this, source);
	        this.schemaVersion = source["schemaVersion"];
	        this.exportedAt = source["exportedAt"];
	        this.recording = this.convertValues(source["recording"], Recording);
	        this.recordings = this.convertValues(source["recordings"], Recording);
	    }

		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	export class RecordingEventStats {
	    total: number;
	    move: number;
	    click: number;
	    key: number;
	    scroll: number;

	    static createFrom(source: any = {}) {
	        return new RecordingEventStats(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.total = source["total"];
	        this.move = source["move"];
	        this.click = source["click"];
	        this.key = source["key"];
	        this.scroll = source["scroll"];
	    }
	}
	export class RecordingSummary {
	    id: string;
	    name: string;
	    description: string;
	    eventCount: number;
	    durationMs: number;
	    viewportW: number;
	    viewportH: number;
	    startUrl?: string;
	    currentUrl?: string;
	    title?: string;
	    devicePixelRatio?: number;
	    scale?: number;
	    createdAt: string;

	    static createFrom(source: any = {}) {
	        return new RecordingSummary(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.id = source["id"];
	        this.name = source["name"];
	        this.description = source["description"];
	        this.eventCount = source["eventCount"];
	        this.durationMs = source["durationMs"];
	        this.viewportW = source["viewportW"];
	        this.viewportH = source["viewportH"];
	        this.startUrl = source["startUrl"];
	        this.currentUrl = source["currentUrl"];
	        this.title = source["title"];
	        this.devicePixelRatio = source["devicePixelRatio"];
	        this.scale = source["scale"];
	        this.createdAt = source["createdAt"];
	    }
	}
	export class RecordingDetailPage {
	    recording?: RecordingSummary;
	    events: RecordedEvent[];
	    eventOffset: number;
	    eventLimit: number;
	    eventTotal: number;
	    stats: RecordingEventStats;

	    static createFrom(source: any = {}) {
	        return new RecordingDetailPage(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.recording = this.convertValues(source["recording"], RecordingSummary);
	        this.events = this.convertValues(source["events"], RecordedEvent);
	        this.eventOffset = source["eventOffset"];
	        this.eventLimit = source["eventLimit"];
	        this.eventTotal = source["eventTotal"];
	        this.stats = this.convertValues(source["stats"], RecordingEventStats);
	    }

		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}



	export class VariationConfig {
	    intensity: number;
	    timingJitter: number;
	    positionJitter: number;
	    speedVariation: number;
	    microCorrections: boolean;
	    extraPauses: boolean;

	    static createFrom(source: any = {}) {
	        return new VariationConfig(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.intensity = source["intensity"];
	        this.timingJitter = source["timingJitter"];
	        this.positionJitter = source["positionJitter"];
	        this.speedVariation = source["speedVariation"];
	        this.microCorrections = source["microCorrections"];
	        this.extraPauses = source["extraPauses"];
	    }
	}

}

export namespace browser {

	export class CoreExtendedInfo {
	    coreId: string;
	    chromeVersion: string;
	    instanceCount: number;

	    static createFrom(source: any = {}) {
	        return new CoreExtendedInfo(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.coreId = source["coreId"];
	        this.chromeVersion = source["chromeVersion"];
	        this.instanceCount = source["instanceCount"];
	    }
	}
	export class CoreInput {
	    coreId: string;
	    coreName: string;
	    corePath: string;
	    kind?: string;
	    isDefault: boolean;

	    static createFrom(source: any = {}) {
	        return new CoreInput(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.coreId = source["coreId"];
	        this.coreName = source["coreName"];
	        this.corePath = source["corePath"];
	        this.kind = source["kind"];
	        this.isDefault = source["isDefault"];
	    }
	}
	export class CoreValidateResult {
	    valid: boolean;
	    message: string;

	    static createFrom(source: any = {}) {
	        return new CoreValidateResult(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.valid = source["valid"];
	        this.message = source["message"];
	    }
	}
	export class Group {
	    groupId: string;
	    groupName: string;
	    parentId: string;
	    sortOrder: number;
	    createdAt: string;
	    updatedAt: string;

	    static createFrom(source: any = {}) {
	        return new Group(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.groupId = source["groupId"];
	        this.groupName = source["groupName"];
	        this.parentId = source["parentId"];
	        this.sortOrder = source["sortOrder"];
	        this.createdAt = source["createdAt"];
	        this.updatedAt = source["updatedAt"];
	    }
	}
	export class GroupInput {
	    groupName: string;
	    parentId: string;
	    sortOrder: number;

	    static createFrom(source: any = {}) {
	        return new GroupInput(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.groupName = source["groupName"];
	        this.parentId = source["parentId"];
	        this.sortOrder = source["sortOrder"];
	    }
	}
	export class GroupWithCount {
	    groupId: string;
	    groupName: string;
	    parentId: string;
	    sortOrder: number;
	    createdAt: string;
	    updatedAt: string;
	    instanceCount: number;

	    static createFrom(source: any = {}) {
	        return new GroupWithCount(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.groupId = source["groupId"];
	        this.groupName = source["groupName"];
	        this.parentId = source["parentId"];
	        this.sortOrder = source["sortOrder"];
	        this.createdAt = source["createdAt"];
	        this.updatedAt = source["updatedAt"];
	        this.instanceCount = source["instanceCount"];
	    }
	}
	export class Profile {
	    profileId: string;
	    profileName: string;
	    userDataDir: string;
	    coreId: string;
	    fingerprintArgs: string[];
	    preferencesOverrides?: Record<string, any>;
	    proxyId: string;
	    proxyConfig: string;
	    proxyBindSourceId: string;
	    proxyBindSourceUrl: string;
	    proxyBindName: string;
	    proxyBindUpdatedAt: string;
	    launchArgs: string[];
	    tags: string[];
	    keywords: string[];
	    groupId: string;
	    launchCode: string;
	    behaviorProfileId: string;
	    running: boolean;
	    debugPort: number;
	    debugReady: boolean;
	    pid: number;
	    runtimeWarning: string;
	    lastError: string;
	    createdAt: string;
	    updatedAt: string;
	    lastStartAt: string;
	    lastStopAt: string;

	    static createFrom(source: any = {}) {
	        return new Profile(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.profileId = source["profileId"];
	        this.profileName = source["profileName"];
	        this.userDataDir = source["userDataDir"];
	        this.coreId = source["coreId"];
	        this.fingerprintArgs = source["fingerprintArgs"];
	        this.preferencesOverrides = source["preferencesOverrides"];
	        this.proxyId = source["proxyId"];
	        this.proxyConfig = source["proxyConfig"];
	        this.proxyBindSourceId = source["proxyBindSourceId"];
	        this.proxyBindSourceUrl = source["proxyBindSourceUrl"];
	        this.proxyBindName = source["proxyBindName"];
	        this.proxyBindUpdatedAt = source["proxyBindUpdatedAt"];
	        this.launchArgs = source["launchArgs"];
	        this.tags = source["tags"];
	        this.keywords = source["keywords"];
	        this.groupId = source["groupId"];
	        this.launchCode = source["launchCode"];
	        this.behaviorProfileId = source["behaviorProfileId"];
	        this.running = source["running"];
	        this.debugPort = source["debugPort"];
	        this.debugReady = source["debugReady"];
	        this.pid = source["pid"];
	        this.runtimeWarning = source["runtimeWarning"];
	        this.lastError = source["lastError"];
	        this.createdAt = source["createdAt"];
	        this.updatedAt = source["updatedAt"];
	        this.lastStartAt = source["lastStartAt"];
	        this.lastStopAt = source["lastStopAt"];
	    }
	}
	export class ProfileInput {
	    profileName: string;
	    userDataDir: string;
	    coreId: string;
	    fingerprintArgs: string[];
	    preferencesOverrides?: Record<string, any>;
	    proxyId: string;
	    proxyConfig: string;
	    launchArgs: string[];
	    tags: string[];
	    keywords: string[];
	    groupId: string;
	    behaviorProfileId: string;

	    static createFrom(source: any = {}) {
	        return new ProfileInput(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.profileName = source["profileName"];
	        this.userDataDir = source["userDataDir"];
	        this.coreId = source["coreId"];
	        this.fingerprintArgs = source["fingerprintArgs"];
	        this.preferencesOverrides = source["preferencesOverrides"];
	        this.proxyId = source["proxyId"];
	        this.proxyConfig = source["proxyConfig"];
	        this.launchArgs = source["launchArgs"];
	        this.tags = source["tags"];
	        this.keywords = source["keywords"];
	        this.groupId = source["groupId"];
	        this.behaviorProfileId = source["behaviorProfileId"];
	    }
	}
	export class Settings {
	    userDataRoot: string;
	    defaultFingerprintArgs: string[];
	    defaultLaunchArgs: string[];
	    defaultProxy: string;
	    startReadyTimeoutMs: number;
	    startStableWindowMs: number;

	    static createFrom(source: any = {}) {
	        return new Settings(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.userDataRoot = source["userDataRoot"];
	        this.defaultFingerprintArgs = source["defaultFingerprintArgs"];
	        this.defaultLaunchArgs = source["defaultLaunchArgs"];
	        this.defaultProxy = source["defaultProxy"];
	        this.startReadyTimeoutMs = source["startReadyTimeoutMs"];
	        this.startStableWindowMs = source["startStableWindowMs"];
	    }
	}
	export class Tab {
	    tabId: string;
	    title: string;
	    url: string;
	    active: boolean;

	    static createFrom(source: any = {}) {
	        return new Tab(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.tabId = source["tabId"];
	        this.title = source["title"];
	        this.url = source["url"];
	        this.active = source["active"];
	    }
	}

}

export namespace config {

	export class BrowserBookmark {
	    name: string;
	    url: string;

	    static createFrom(source: any = {}) {
	        return new BrowserBookmark(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.name = source["name"];
	        this.url = source["url"];
	    }
	}
	export class BrowserCore {
	    coreId: string;
	    coreName: string;
	    corePath: string;
	    kind?: string;
	    isDefault: boolean;

	    static createFrom(source: any = {}) {
	        return new BrowserCore(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.coreId = source["coreId"];
	        this.coreName = source["coreName"];
	        this.corePath = source["corePath"];
	        this.kind = source["kind"];
	        this.isDefault = source["isDefault"];
	    }
	}
	export class BrowserProxy {
	    proxyId: string;
	    proxyName: string;
	    proxyConfig: string;
	    dnsServers?: string;
	    groupName?: string;
	    sortOrder?: number;
	    sourceId?: string;
	    sourceUrl?: string;
	    sourceNamePrefix?: string;
	    sourceAutoRefresh?: boolean;
	    sourceRefreshIntervalM?: number;
	    sourceLastRefreshAt?: string;
	    lastLatencyMs: number;
	    lastTestOk: boolean;
	    lastTestedAt: string;
	    lastIPHealthJson?: string;

	    static createFrom(source: any = {}) {
	        return new BrowserProxy(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.proxyId = source["proxyId"];
	        this.proxyName = source["proxyName"];
	        this.proxyConfig = source["proxyConfig"];
	        this.dnsServers = source["dnsServers"];
	        this.groupName = source["groupName"];
	        this.sortOrder = source["sortOrder"];
	        this.sourceId = source["sourceId"];
	        this.sourceUrl = source["sourceUrl"];
	        this.sourceNamePrefix = source["sourceNamePrefix"];
	        this.sourceAutoRefresh = source["sourceAutoRefresh"];
	        this.sourceRefreshIntervalM = source["sourceRefreshIntervalM"];
	        this.sourceLastRefreshAt = source["sourceLastRefreshAt"];
	        this.lastLatencyMs = source["lastLatencyMs"];
	        this.lastTestOk = source["lastTestOk"];
	        this.lastTestedAt = source["lastTestedAt"];
	        this.lastIPHealthJson = source["lastIPHealthJson"];
	    }
	}

}

export namespace events {

	export class EventLogEntry {
	    id: number;
	    eventName: string;
	    namespace: string;
	    severity: string;
	    payload: Record<string, any>;
	    createdAt: string;

	    static createFrom(source: any = {}) {
	        return new EventLogEntry(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.id = source["id"];
	        this.eventName = source["eventName"];
	        this.namespace = source["namespace"];
	        this.severity = source["severity"];
	        this.payload = source["payload"];
	        this.createdAt = source["createdAt"];
	    }
	}

}

export namespace launchcode {

	export class LaunchRequestParams {
	    launchArgs: string[];
	    startUrls: string[];
	    skipDefaultStartUrls: boolean;

	    static createFrom(source: any = {}) {
	        return new LaunchRequestParams(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.launchArgs = source["launchArgs"];
	        this.startUrls = source["startUrls"];
	        this.skipDefaultStartUrls = source["skipDefaultStartUrls"];
	    }
	}
	export class WorkbenchWindowPlacement {
	    profileId: string;
	    profileName: string;
	    pid: number;
	    found: boolean;
	    x: number;
	    y: number;
	    width: number;
	    height: number;
	    error?: string;

	    static createFrom(source: any = {}) {
	        return new WorkbenchWindowPlacement(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.profileId = source["profileId"];
	        this.profileName = source["profileName"];
	        this.pid = source["pid"];
	        this.found = source["found"];
	        this.x = source["x"];
	        this.y = source["y"];
	        this.width = source["width"];
	        this.height = source["height"];
	        this.error = source["error"];
	    }
	}

}

export namespace logger {

	export class MemoryLogEntry {
	    time: string;
	    level: string;
	    component: string;
	    message: string;
	    fields?: Record<string, any>;

	    static createFrom(source: any = {}) {
	        return new MemoryLogEntry(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.time = source["time"];
	        this.level = source["level"];
	        this.component = source["component"];
	        this.message = source["message"];
	        this.fields = source["fields"];
	    }
	}
	export class MethodInterceptor {


	    static createFrom(source: any = {}) {
	        return new MethodInterceptor(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);

	    }
	}

}
