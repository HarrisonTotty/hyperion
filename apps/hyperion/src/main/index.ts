import { join } from "node:path";

import { app, BrowserWindow, shell } from "electron";

import { isSafeExternalUrl, isSameDocument } from "./navigation";

function reportLoadFailure(error: unknown): void {
  console.error("failed to load the renderer:", error);
}

function createWindow(): void {
  const window = new BrowserWindow({
    width: 1600,
    height: 900,
    minWidth: 1024,
    minHeight: 640,
    show: false,
    backgroundColor: "#05080d",
    autoHideMenuBar: true,
    webPreferences: {
      preload: join(__dirname, "../preload/index.js"),
      sandbox: true,
      contextIsolation: true,
      nodeIntegration: false,
    },
  });

  window.once("ready-to-show", () => {
    window.show();
  });

  // Never open foreign pages inside the bridge; hand web links to the OS browser.
  window.webContents.setWindowOpenHandler(({ url }) => {
    if (isSafeExternalUrl(url)) {
      shell.openExternal(url).catch((error: unknown) => {
        console.error("failed to open external link:", error);
      });
    } else {
      console.warn("blocked attempt to open unsafe url:", url);
    }
    return { action: "deny" };
  });

  // The bridge is a single page: anything but a reload would replace it with foreign content.
  window.webContents.on("will-navigate", (event, url) => {
    if (!isSameDocument(url, window.webContents.getURL())) {
      event.preventDefault();
      console.warn("blocked navigation to:", url);
    }
  });

  // electron-vite serves the renderer over HTTP in dev and from disk in production.
  const devServerUrl = process.env["ELECTRON_RENDERER_URL"];
  if (!app.isPackaged && devServerUrl !== undefined) {
    window.loadURL(devServerUrl).catch(reportLoadFailure);
  } else {
    window.loadFile(join(__dirname, "../renderer/index.html")).catch(reportLoadFailure);
  }
}

async function main(): Promise<void> {
  await app.whenReady();
  createWindow();

  app.on("activate", () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createWindow();
    }
  });
}

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") {
    app.quit();
  }
});

main().catch((error: unknown) => {
  console.error("failed to start:", error);
  app.exit(1);
});
