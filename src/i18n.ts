import { ref } from "vue";

export type AppLocale = "en" | "zh-CN";
export const localeStorageKey = "yingxiang-video.locale";

// English is the base language. Regional Chinese tags share Simplified Chinese.
export function getStremioLanguage(language: string): AppLocale {
  return /^zh(?:-|$)/i.test(language.replaceAll("_", "-")) ? "zh-CN" : "en";
}

function readLocale(): AppLocale {
  if (typeof window === "undefined") return "en";
  try {
    return getStremioLanguage(localStorage.getItem(localeStorageKey) || "en");
  } catch {
    return "en";
  }
}

export const locale = ref<AppLocale>(readLocale());
export function setLocale(value: AppLocale): boolean {
  locale.value = value;
  try {
    localStorage.setItem(localeStorageKey, value);
    return true;
  } catch {
    return false;
  }
}

export const chinese = {
  "Remove from history": "删除观看记录",
  "Remove": "删除",
  "Remove {title} and its saved progress from this device?": "删除此设备中《{title}》的观看记录和播放进度？",
  "Watch history": "观看历史",
  "History": "历史",
  "Today": "今天",
  "Yesterday": "昨天",
  "Earlier": "更早",
  "Your stories, ready to continue.": "从上次停下的地方，继续故事。",
  "Clear history": "清空历史",
  "Clear all watch history? This removes saved progress from this device.": "清空全部观看历史？此设备保存的播放进度将一并删除。",
  "Watch history could not be saved": "观看历史保存失败",
  "History is unavailable": "观看历史暂不可用",
  "Retry": "重试",
  "Loading watch history": "正在读取观看历史",
  "Your next story starts here.": "这里将留下你看过的故事。",
  "Watch a video and your progress will appear here automatically.": "开始播放影片后，观看记录和进度会自动保存在这里。",
  "1 watched title": "共 1 部观看记录",
  "Preview history resets when you reload this page.": "当前为界面预览，刷新页面后观看记录会重置。",
  "{count} watched titles": "共 {count} 部观看记录",
  "Episode finished": "本集已看完",
  "Completed": "已看完",
  "Play again": "重新播放",
  "Continue watching": "继续观看",
  "Remove {title} from history": "删除《{title}》的观看记录",
  "Preparing saved progress…": "正在恢复播放进度…",
  "Saved progress could not be restored": "播放进度恢复失败",
  "This title is unavailable in the current catalog.": "当前目录暂未找到该影片，请刷新影片目录。",
  "The saved playback source is unavailable. Enable it in Sources before continuing.": "原播放源暂不可用，请先在来源管理中启用后继续。",
  "The saved episode is unavailable. Open the film details to choose an episode.": "原播放集数暂不可用，请到影片详情重新选择集数。",
  "No saved progress was found for this episode.": "该集的观看记录已更新或删除，请从历史页重新选择。",

  Appearance: "外观",
  "Customize colors": "自定义色系",
  "Display mode": "显示模式",
  "Light and dark mode automatically follow your system.":
    "明暗模式自动跟随系统设置。",
  "Following system · {mode}": "跟随系统 · {mode}",
  "Color palette": "色系",
  "Choose your colors. Each palette adapts to light and dark mode.":
    "选择喜欢的色系，每种色系均适配浅色与深色模式。",
  "Color palette changed, but the preference could not be saved on this device.":
    "已切换色系，此设备暂未能保存偏好。",
  Lime: "青柠",
  Ocean: "海蓝",
  Violet: "鸢紫",
  Rose: "玫瑰",
  Amber: "琥珀",
  "Light mode": "浅色模式",
  "Dark mode": "深色模式",
  "Skip to content": "跳到主要内容",
  "Main navigation": "主导航",
  "Yingxiang Video home": "映象视频首页",
  "Yingxiang Video": "映象视频",
  "DISCOVER GOOD STORIES": "去发现好故事",
  "YOUR SCREENING ROOM": "你的放映室",
  "Source manager": "来源管理",
  Settings: "播放设置",
  "Make good stories": "让好故事",
  "part of every day.": "成为日常的一部分。",
  "Interface preview": "界面预览",
  "Yingxiang Video · 0.1.0": "映象视频 · 0.1.0",
  Back: "返回",
  "Make time for a good story.": "把时间，留给好故事。",
  Demo: "演示",
  "Choose a video source": "选择影片来源",
  "Manage all sources": "管理全部来源",
  "Open settings": "打开播放设置",
  "Find a story for the way you feel.": "总有一个故事，与你此刻同频。",
  "Editor's pick": "编辑精选",
  "Play now": "立即播放",
  "Film details": "影片详情",
  "A journey of departures and new beginnings": "一场关于出发与抵达的想象",
  "Previous feature": "上一部精选",
  "Next feature": "下一部精选",
  "Start your next screening with a story worth seeing.":
    "从一部好作品，开始今天的放映。",
  "Filter by genre": "按题材筛选",
  "Switching demo catalog": "正在切换演示目录",
  "A new story is on its way": "这个角落，等一个新故事",
  "View all titles": "查看全部作品",
  "See the world a little differently.": "偶尔，也换一种看世界的方式。",
  "Step outside the everyday and into a new story.":
    "离开熟悉的日常，走进一个全新的故事。",
  "Explore animation": "去动漫放映室",
  Director: "导演",
  Cast: "主演",
  "You might also like": "也许，你还会喜欢",
  "Something to look forward to next.": "给下一场放映，留一点期待。",
  "Local demo clip · 00:30": "本地演示片段 · 00:30",
  Chinese: "中文",
  "Preparing video…": "正在准备画面…",
  "The clip could not be loaded": "片段暂时未能加载",
  "Check that the local preview server is running, then try again.":
    "请确认本地预览服务正在运行，然后重试。",
  Reload: "重新加载",
  "Playback progress": "播放进度",
  "Next episode": "下一集",
  Volume: "音量",
  "Playback speed": "播放速度",
  "Toggle subtitles": "切换字幕",
  Fullscreen: "全屏",
  "Playing an original test clip. All film and credit information is fictional.":
    "正在播放原创画面测试片段，影片与演职员信息均为虚构。",
  "Auto-play next": "自动下一集",
  "Playback streams": "播放线路",
  "Default stream": "默认线路",
  "Alternative stream": "备用线路",
  Episodes: "选集",
  "Good stories start here.": "好故事，从这里连接。",
  "Choose a source for your next screening.":
    "选择一个来源，开启你的下一场放映。",
  "Add source": "添加来源",
  "This preview uses demo catalogs. Added sources stay in this page session; live connections will follow.":
    "当前为界面预览，来源切换使用演示目录。新增配置仅保留在本次页面会话中，实际接口将在后续接入。",
  "Current source": "当前来源",
  "Use source": "使用此来源",
  "MacCMS JSON, Stremio add-ons and Cloudstream share one browsing and playback experience.":
    "支持 MacCMS JSON、Stremio 扩展与 Cloudstream，统一浏览与播放体验。",
  "Make yourself comfortable.": "按你喜欢的方式放映。",
  "A few small adjustments for a better viewing experience.":
    "一点小调整，让每一场观看更自在。",
  "Playback preferences": "播放偏好",
  "Automatically play next episode": "自动播放下一集",
  "Continue to the next episode when this one ends.":
    "剧集播放结束后，继续下一集。",
  "Default playback speed": "默认播放速度",
  "Use this speed for all local demo clips.": "所有本地演示片段使用此速度。",
  "About Yingxiang Video": "关于映象视频",
  "A place for good stories.": "一处留给好故事的空间。",
  "Interface preview 0.1.0": "界面预览 0.1.0",
  "Explore discovery, film details, episodes, embedded playback and source management. Artwork is original, playback uses a local 30-second demo, and playback preferences last for this page session.":
    "本次交付包含发现、影片详情、选集、内嵌播放与来源管理界面。素材为原创生成画面，播放使用本地 30 秒演示片段；播放偏好保留在当前页面会话中。",
  "Let stories happen": "让故事发生",
  "Interface preview · Original fictional content": "界面预览 · 原创示意内容",
  "Mobile navigation": "移动导航",
  Sources: "来源",
  "Add a video source": "添加影片来源",
  "Close add source": "关闭添加来源",
  "Source configurations stay in this preview session so you can try the management flow.":
    "配置将在当前界面预览中保留，方便体验来源管理流程。",
  "Source name": "来源名称",
  "For example: My film library": "例如：我的电影片库",
  "Source type": "来源类型",
  "Stremio add-on": "Stremio 扩展",
  "API address": "接口地址",
  "For Stremio, enter the full URL ending in manifest.json.":
    "Stremio 请填写以 manifest.json 结尾的完整地址。",
  Cancel: "取消",
  "Welcome to Yingxiang Video": "欢迎来到映象视频",
  "Close preview information": "关闭预览说明",
  "This is an interactive interface preview. Films, credits and synopses are fictional, artwork is AI-generated, and the player uses a local 30-second test clip. Explore details, episodes, subtitles, playback preferences and source management.":
    "这是一份可以操作的界面预览。影片、演职员和简介均为虚构，画面由 AI 原创生成；播放器使用本地 30 秒测试片段。你可以浏览详情、切换集数与字幕、调整播放设置，或体验来源管理。",
  "Start exploring": "开始发现",
  Discover: "发现",
  "Now playing": "正在播放",
  Movies: "电影",
  Series: "剧集",
  Animation: "动漫",
  All: "全部",
  Drama: "剧情",
  Adventure: "冒险",
  Mystery: "悬疑",
  "Sci-fi": "科幻",
  "Feel-good": "治愈",
  Documentary: "纪录片",
  "Demo catalog": "演示目录",
  "Connection pending": "待接入",
  "What will you watch tonight?": "今晚，看点什么？",
  "Selected for you": "精选推荐",
  "Show fewer": "收起影片",
  "All titles": "全部影片",
  "Start episode 1": "从第 1 集开始",
  episodes: "集数",
  "a video": "播放内容",
  Feature: "正片",
  Replay: "重新播放",
  Play: "播放",
  Pause: "暂停",
  Unmute: "取消静音",
  Mute: "静音",
  Disabled: "已停用",
  "Demo source": "演示来源",
  "Added · Connection pending": "已添加 · 待接入",
  Disable: "停用",
  Enable: "启用",
  "Enter a complete HTTP or HTTPS URL without credentials or a fragment.":
    "请输入完整的 HTTP 或 HTTPS 接口地址，移除账户信息和片段。",
  "The Stremio add-on URL must end in /manifest.json.":
    "Stremio 扩展地址需要以 /manifest.json 结尾。",
  "Enter a name for this source.": "请为这个来源起一个名称。",
  "Source added to this preview. Live connections will follow.":
    "来源已加入本次预览，实际接口将在后续接入。",
  "Enter a valid full URL, such as https://example.com/api.php/provide/vod/.":
    "地址格式有误，请填写完整地址，例如 https://example.com/api.php/provide/vod/。",
  "Switch to another source before disabling the current one.":
    "当前来源正在使用，请先切换到其他来源。",
  "Source enabled": "来源已启用",
  "Source disabled": "来源已停用",
  "Fullscreen is unavailable here. Try maximizing the window.":
    "当前环境暂不支持全屏，请使用窗口最大化。",
  "Selected {name} · Using the demo catalog":
    "已选择 {name} · 当前使用演示目录",
  "Selected {name} · Both streams use the same local demo clip":
    "已选择 {name} · 两条线路使用同一本地演示片段",
  "Take your time with good {kind}.": "好{kind}，慢慢看。",
  "The {kind} room": "{kind}放映室",
  "{count} titles": "{count} 部作品",
  "There are no {genre} titles in this demo catalog yet. Try another genre.":
    "当前演示目录还没有{genre}作品，换个题材看看。",
  "Illustrative poster for {title}": "{title}示意海报",
  "{count} episodes": "{count} 集",
  "Choose {kind}": "选择{kind}",
  "Source: {name} · Local demo clip": "当前来源：{name} · 本地演示片段",
  "{count} episodes in total": "共 {count} 集",
  "1 feature": "1 部正片",
  "Episode {number}": "第 {number} 集",
  "View details for {title}": "查看《{title}》详情",
  "App language": "界面语言",
  "Language & metadata": "语言与元数据",
  "Change the interface language and save it on this device.":
    "立即切换页面语言，并保存在此设备。",
  "Metadata provider": "元数据来源",
  "Metadata language": "元数据语言",
  "Follows the app: Chinese uses Chinese; all other languages use English.":
    "跟随页面：中文使用中文，其余使用英文。",
  "Simplified Chinese": "简体中文",
  English: "英文",
  "Language changed, but the preference could not be saved on this device. Reloading will restore English.":
    "已切换语言；此设备未能保存偏好，刷新后将恢复英文。",
  "Metadata language preview · Live Stremio connection pending":
    "元数据语言预览 · 真实 Stremio 接口待接入",
} as const;
export type Message = keyof typeof chinese;

export function t(
  message: Message,
  values: Record<string, string | number> = {},
): string {
  const text = locale.value === "zh-CN" ? chinese[message] : message;
  return text.replace(/\{(\w+)\}/g, (token, key: string) =>
    String(values[key] ?? token),
  );
}
