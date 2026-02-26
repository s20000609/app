# UI 翻譯 (i18n) 說明

## 已完成的設定

- **套件**：`i18next`、`react-i18next`（需執行 `npm install` 安裝）
- **語系檔**：`src/locales/en.json`（英文）、`src/locales/zh-Hant.json`（繁體中文）
- **初始化**：`src/i18n.ts` 會在 `main.tsx` 載入時執行，並從 `localStorage` 讀取 `app-locale`（`en` 或 `zh-Hant`）
- **切換語系**：設定 → **Accessibility（無障礙）** 頁面最上方「Language」下拉選單，可選 English / 繁體中文

## 已翻譯的 UI

- **頂部導覽標題**（`TopNav`）：設定、模型、對話、資源庫等頁面標題已改為使用 `common.nav.*` 鍵值，會依目前語系顯示英文或繁中。

## 如何逐步翻譯其他畫面

### 1. 在語系檔加上鍵值

在 `src/locales/en.json` 與 `src/locales/zh-Hant.json` 的同一路徑下新增同一 key，例如：

```json
// en.json
{
  "common": {
    "nav": { ... },
    "settings": {
      "save": "Save",
      "cancel": "Cancel"
    }
  }
}

// zh-Hant.json
{
  "common": {
    "settings": {
      "save": "儲存",
      "cancel": "取消"
    }
  }
}
```

### 2. 在元件裡用 `useTranslation` 與 `t()`

```tsx
import { useTranslation } from "react-i18next";

export function SomePage() {
  const { t } = useTranslation();
  return (
    <div>
      <button>{t("common.settings.save")}</button>
      <button>{t("common.settings.cancel")}</button>
    </div>
  );
}
```

### 3. 帶變數的翻譯

語系檔：

```json
"greeting": "Hello, {{name}}!"
```

元件：

```tsx
t("greeting", { name: "User" })
```

### 4. 建議的鍵值結構

- `common.*`：通用（按鈕、標籤、導覽）
- `settings.*`：設定頁專用
- `chat.*`：對話/聊天相關
- `onboarding.*`：歡迎/引導流程

可依功能再分子物件，例如 `common.nav`、`settings.embedding`。

## 注意事項

- 若 key 不存在，`t("some.key")` 會回傳 key 本身，方便開發時發現漏翻。
- 切換語系後會寫入 `localStorage` 的 `app-locale`，下次啟動會沿用。
- 新增語系（例如日文）：在 `src/locales/` 加 `ja.json`，在 `src/i18n.ts` 的 `resources` 與 `LOCALE_OPTIONS`（在 Accessibility 頁）加入對應選項即可。
