import { test } from "node:test";
import assert from "node:assert/strict";
import {
  chinese,
  getStremioLanguage,
  locale,
  localeStorageKey,
  setLocale,
  t,
} from "../src/i18n.ts";

test("Chinese variants map to Chinese metadata; every other language uses English", () => {
  for (const value of ["zh", "zh-CN", "zh-TW", "zh-Hant-HK", "ZH_cn"]) {
    assert.equal(getStremioLanguage(value), "zh-CN", value);
  }
  for (const value of [
    "en",
    "en-GB",
    "ja",
    "fr-FR",
    "de",
    "",
    "zho",
    "zhang",
  ]) {
    assert.equal(getStremioLanguage(value), "en", value);
  }
});

test("English is the default and translated messages preserve placeholder values", () => {
  assert.equal(locale.value, "en");
  assert.equal(
    t("View details for {title}", { title: "A Film" }),
    "View details for A Film",
  );
  locale.value = "zh-CN";
  assert.equal(
    t("View details for {title}", { title: "A Film" }),
    "查看《A Film》详情",
  );
  assert.equal(t("Episode {number}", { number: 4 }), "第 4 集");
  for (const [english, translation] of Object.entries(chinese)) {
    const tokens = (value: string) =>
      [...value.matchAll(/\{\w+\}/g)].map((item) => item[0]).sort();
    assert.deepEqual(tokens(translation), tokens(english), english);
  }
  locale.value = "en";
});

test("Language preference is saved; unavailable storage reports a failure and keeps the selected language", () => {
  let saved: [string, string] | undefined;
  Object.defineProperty(globalThis, "localStorage", {
    configurable: true,
    value: {
      setItem(key: string, value: string) {
        saved = [key, value];
      },
    },
  });
  assert.equal(setLocale("zh-CN"), true);
  assert.deepEqual(saved, [localeStorageKey, "zh-CN"]);
  Object.defineProperty(globalThis, "localStorage", {
    configurable: true,
    value: {
      setItem() {
        throw new Error("Storage disabled");
      },
    },
  });
  assert.equal(setLocale("en"), false);
  assert.equal(locale.value, "en");
  Reflect.deleteProperty(globalThis, "localStorage");
});
