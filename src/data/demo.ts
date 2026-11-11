import { locale } from "../i18n";

export type FilmKind = "movie" | "series" | "anime";
export type SourceKind = "maccms" | "stremio" | "cloudstream";

export interface Film {
  id: string;
  title: string;
  english: string;
  kind: FilmKind;
  year: number;
  genres: string[];
  runtime: string;
  plot: string;
  note: string;
  artwork: string;
  position?: string;
  episodes: number;
  director: string;
  cast: string[];
  posterTone?: string;
}

export interface Source {
  id: string;
  name: string;
  kind: SourceKind;
  description: string;
  url?: string;
  enabled: boolean;
  demo: boolean;
}

export const kindLabels = {
  movie: "Movies",
  series: "Series",
  anime: "Animation",
} as const;
export const sourceLabels: Record<SourceKind, string> = {
  maccms: "MacCMS",
  stremio: "Stremio",
  cloudstream: "Cloudstream",
};

// Original, fictional fixtures. IDs and genre values stay stable across languages.
export const films: Film[] = [
  {
    id: "wilderness",
    title: "The Quiet Frontier",
    english: "THE QUIET FRONTIER",
    kind: "movie",
    year: 2026,
    genres: ["Drama", "Adventure"],
    runtime: "1h 42m",
    plot: "One autumn, Lin Yuan leaves his life on pause and sets out alone for the north. Valleys, lakes and chance encounters help him make peace with silence and find a reason to begin again.",
    note: "Some answers arrive on the wind.",
    artwork: "/artwork/wilderness.webp",
    position: "73% center",
    episodes: 1,
    director: "Lin Xu",
    cast: ["Chen Yu", "Xu Zhiyao"],
  },
  {
    id: "midnight",
    title: "Letters After Midnight",
    english: "LETTERS AFTER MIDNIGHT",
    kind: "series",
    year: 2026,
    genres: ["Mystery", "Drama"],
    runtime: "45m per episode",
    plot: "An unsigned letter connects a rainy night in the old town with a forgotten past. Archive restorer Shen Dong searches for the sender, only to find another question behind every answer.",
    note: "The story begins when the city sleeps.",
    artwork: "/artwork/city.webp",
    episodes: 12,
    director: "Zhou Jianchuan",
    cast: ["Su Nan", "Jiang Chi"],
  },
  {
    id: "summer",
    title: "A Summer To Stay",
    english: "A SUMMER TO STAY",
    kind: "movie",
    year: 2025,
    genres: ["Drama", "Feel-good"],
    runtime: "1h 36m",
    plot: "Xu Xia returns to her childhood seaside town to sort through her grandmother’s cottage. Tides, old photographs and the neighbours turn a short visit into a summer worth staying for.",
    note: "A summer to find the shape of life again.",
    artwork: "/artwork/coast.webp",
    episodes: 1,
    director: "Xu Qinghe",
    cast: ["Ye Shian", "Song Yu"],
    posterTone: "warm",
  },
  {
    id: "moon",
    title: "Almost The Moon",
    english: "ALMOST THE MOON",
    kind: "movie",
    year: 2026,
    genres: ["Sci-fi", "Adventure"],
    runtime: "1h 58m",
    plot: "A lunar observatory receives a recording from the old Earth. Its lone astronaut follows the signal across silent craters, searching the distant stars for a way home.",
    note: "Across the universe, longing has an echo.",
    artwork: "/artwork/space.webp",
    episodes: 1,
    director: "Lu Xingye",
    cast: ["Gu Bei", "Lin Mi"],
  },
  {
    id: "train",
    title: "The Summer Express",
    english: "THE SUMMER EXPRESS",
    kind: "anime",
    year: 2025,
    genres: ["Feel-good", "Adventure"],
    runtime: "1h 28m",
    plot: "On the first day of summer, Xiaoman boards a slow train that only appears in sunshine. Along the coast and through the forest, a letter awaits delivery and a quiet courage begins to grow.",
    note: "Next stop: a summer still to come.",
    artwork: "/artwork/train.webp",
    episodes: 1,
    director: "Mountain Studio",
    cast: ["Xiaoman", "A He"],
  },
  {
    id: "mountains",
    title: "Between Land And Sea",
    english: "BETWEEN LAND AND SEA",
    kind: "series",
    year: 2025,
    genres: ["Drama", "Feel-good"],
    runtime: "40m per episode",
    plot: "Three young people from different cities run a small inn at the foot of a mountain. Guests bring news from afar as the seasons change, helping them rediscover themselves and each other.",
    note: "Slow down and let the world come closer.",
    artwork: "/artwork/wilderness.webp",
    position: "40% center",
    episodes: 8,
    director: "Chen Jian",
    cast: ["Xu Zhiyao", "Liang Mu"],
    posterTone: "soft",
  },
  {
    id: "last-rain",
    title: "The Last Night Bus",
    english: "THE LAST NIGHT BUS",
    kind: "movie",
    year: 2024,
    genres: ["Mystery", "Drama"],
    runtime: "1h 47m",
    plot: "On a stormy night, an unexpected passenger boards the last bus. Driver and passenger each carry a secret, and a sudden detour brings their lives together.",
    note: "Someone is always waiting for a light.",
    artwork: "/artwork/city.webp",
    position: "35% center",
    episodes: 1,
    director: "He Yusheng",
    cast: ["Jiang Chi", "Wen Ran"],
    posterTone: "soft",
  },
  {
    id: "blue-holiday",
    title: "Days In Blue",
    english: "DAYS IN BLUE",
    kind: "movie",
    year: 2025,
    genres: ["Feel-good", "Drama"],
    runtime: "1h 32m",
    plot: "An unplanned trip, a room facing the sea and a barista who is always late. On this unhurried island, unfinished things can finally be set gently aside.",
    note: "Make today a day you love.",
    artwork: "/artwork/coast.webp",
    position: "70% center",
    episodes: 1,
    director: "Song Bo",
    cast: ["Ye Shian", "Lu Yao"],
  },
];
const chineseFilms: Record<
  string,
  Pick<Film, "title" | "plot" | "note" | "runtime" | "director" | "cast">
> = {
  wilderness: {
    title: "去有风的旷野",
    plot: "在生活按下暂停键的那个秋天，林远独自踏上了通往北方的路。山谷、湖泊和偶然相遇的人，让他慢慢学会与沉默相处，也重新找到了出发的理由。",
    note: "有些答案，风会带来。",
    runtime: "1 小时 42 分钟",
    director: "林序",
    cast: ["陈屿", "许知遥"],
  },
  midnight: {
    title: "午夜来信",
    plot: "一封没有署名的信，把旧城区的雨夜和一段被遗忘的往事连接起来。档案修复师沈冬开始寻找寄信的人，却发现每个答案背后，都藏着另一个问题。",
    note: "当整座城市入睡，故事才刚刚开始。",
    runtime: "每集 45 分钟",
    director: "周见川",
    cast: ["苏南", "江迟"],
  },
  summer: {
    title: "夏日停留",
    plot: "为了整理祖母留下的海边小屋，许夏回到久别的小镇。在潮汐、旧照片和邻居的日常里，一个原本短暂的夏天，渐渐有了值得留下的理由。",
    note: "借一个夏天，找回生活的形状。",
    runtime: "1 小时 36 分钟",
    director: "许青禾",
    cast: ["叶时安", "宋予"],
  },
  moon: {
    title: "离月球很近",
    plot: "月面观测站收到一段来自旧地球的录音。唯一值守的宇航员沿着信号穿过沉寂的环形山，在遥远的星空下，寻找关于家与归途的线索。",
    note: "宇宙很大，思念有回声。",
    runtime: "1 小时 58 分钟",
    director: "陆星野",
    cast: ["顾北", "林弥"],
  },
  train: {
    title: "夏天的列车",
    plot: "暑假的第一天，小满登上一列只在晴天出现的慢车。沿途的海岸、森林和陌生站台，藏着一封等待送达的信，也藏着她从未察觉的勇气。",
    note: "下一站，是还没遇见的夏天。",
    runtime: "1 小时 28 分钟",
    director: "山间画室",
    cast: ["小满", "阿禾"],
  },
  mountains: {
    title: "山海之间",
    plot: "三个来自不同城市的年轻人，在山脚下经营一家小旅馆。客人带来远方的消息，季节改变门前的风景。他们在平常的日子里，重新认识自己与彼此。",
    note: "停下来，世界会慢慢靠近。",
    runtime: "每集 40 分钟",
    director: "陈间",
    cast: ["许知遥", "梁牧"],
  },
  "last-rain": {
    title: "最后一班雨",
    plot: "暴雨夜，夜班巴士在最后一站多了一位乘客。司机与乘客各自藏着一个秘密，而一条临时改道的路线，让两个人的命运悄悄交汇。",
    note: "夜色深处，总有人在等一盏灯。",
    runtime: "1 小时 47 分钟",
    director: "何雨生",
    cast: ["江迟", "温然"],
  },
  "blue-holiday": {
    title: "蓝色假日",
    plot: "一次没有计划的旅行，一间面朝大海的房间，以及一位总是迟到的咖啡师。在这个缓慢的小岛上，所有未完成的事，终于可以被温柔地放下。",
    note: "把今天，过成喜欢的样子。",
    runtime: "1 小时 32 分钟",
    director: "宋泊",
    cast: ["叶时安", "陆遥"],
  },
};
export function localizeFilm(film: Film): Film {
  return locale.value === "zh-CN"
    ? { ...film, ...chineseFilms[film.id] }
    : film;
}

export const initialSources: Source[] = [
  {
    id: "demo-maccms",
    name: "Light & Shadow",
    kind: "maccms",
    description: "Everyday discoveries in film and television",
    enabled: true,
    demo: true,
  },
  {
    id: "demo-stremio",
    name: "Independent Cinema",
    kind: "stremio",
    description: "Independent stories and new perspectives",
    enabled: true,
    demo: true,
  },
  {
    id: "demo-cloudstream",
    name: "Cloud Pictures",
    kind: "cloudstream",
    description: "A gateway to multiple sources",
    enabled: true,
    demo: true,
  },
];
const chineseSources: Record<string, Pick<Source, "name" | "description">> = {
  "demo-maccms": {
    name: "光影片库",
    description: "电影与剧集的日常发现",
  },
  "demo-stremio": {
    name: "独立电影放映室",
    description: "小众故事与不同视角",
  },
  "demo-cloudstream": {
    name: "云端映像",
    description: "多来源内容入口",
  },
};
export function sourceText(
  source: Source,
): Pick<Source, "name" | "description"> {
  return locale.value === "zh-CN" && source.demo
    ? chineseSources[source.id] || source
    : source;
}
