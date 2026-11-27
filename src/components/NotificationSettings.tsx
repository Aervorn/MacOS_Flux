import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { X } from "lucide-react";
import { api, type NotificationPermission, type Prefs, type SidebarFeed, type SidebarFolder, type SidebarSmartFolder } from "../api";
import styles from "./NotificationSettings.module.css";

type Segmented = <T extends string | number>(value: T, options: { value: T; label: string }[], onPick: (v: T) => void) => React.ReactNode;

interface Props {
  prefs: Prefs;
  onPrefs: (patch: Partial<Prefs>) => void;
  feeds: SidebarFeed[];
  folders: SidebarFolder[];
  smartFolders: SidebarSmartFolder[];
  toggle: (label: string, checked: boolean, onChange: (v: boolean) => void) => React.ReactNode;
  segmented: Segmented;
}

const flip = (list: string[], value: string) => (list.includes(value) ? list.filter((v) => v !== value) : [...list, value]);

/** Réglages › Notifications : quels nouveaux articles signaler, et la barre des menus. */
export default function NotificationSettings({ prefs, onPrefs, feeds, folders, smartFolders, toggle, segmented }: Props) {
  const { t } = useTranslation();
  const [keyword, setKeyword] = useState("");
  const [testResult, setTestResult] = useState<string | null>(null);
  const [permission, setPermission] = useState<NotificationPermission>("unknown");

  // Autorisation de macOS : au chargement, et au retour dans Flux (après un passage dans les Réglages Système)
  useEffect(() => {
    const check = () => void api.notificationPermission().then(setPermission).catch(() => undefined);
    check();
    window.addEventListener("focus", check);
    return () => window.removeEventListener("focus", check);
  }, []);

  const enable = async (on: boolean) => {
    onPrefs({ notifyEnabled: on });
    if (on && permission === "notDetermined") {
      setPermission(await api.requestNotificationPermission().catch(() => "unknown" as const));
    }
  };

  const permissionNotice =
    permission === "denied" ? (
      <div className={styles.alert}>
        <p>{t("notifications.denied")}</p>
        <button type="button" className={styles.button} onClick={() => void api.openNotificationSettings()}>
          {t("notifications.openSettings")}
        </button>
      </div>
    ) : permission === "notDetermined" ? (
      <div className={styles.alert}>
        <p>{t("notifications.notDetermined")}</p>
        <button
          type="button"
          className={styles.button}
          onClick={() => void api.requestNotificationPermission().then(setPermission).catch(() => undefined)}
        >
          {t("notifications.allow")}
        </button>
      </div>
    ) : null;

  const addKeyword = () => {
    const k = keyword.trim();
    if (k && !prefs.notifyKeywords.some((x) => x.toLowerCase() === k.toLowerCase())) {
      onPrefs({ notifyKeywords: [...prefs.notifyKeywords, k] });
    }
    setKeyword("");
  };

  const test = () => {
    api
      .testNotification()
      .then(() => setTestResult(t("notifications.testSent")))
      .catch((e) => setTestResult(String(e)));
  };

  const checklist = (title: string, items: { id: string; label: string }[], selected: string[], onChange: (next: string[]) => void) =>
    items.length > 0 && (
      <div className={styles.group}>
        <span className={styles.label}>{title}</span>
        <div className={styles.list}>
          {items.map((item) => (
            <label key={item.id} className={styles.check}>
              <input type="checkbox" checked={selected.includes(item.id)} onChange={() => onChange(flip(selected, item.id))} />
              {item.label}
            </label>
          ))}
        </div>
      </div>
    );

  const nothingChosen =
    prefs.notifyFeeds.length + prefs.notifyFolders.length + prefs.notifySmartFolders.length + prefs.notifyKeywords.length === 0;

  return (
    <>
      <div className={styles.group}>
        {toggle(t("notifications.enable"), prefs.notifyEnabled, (on) => void enable(on))}
        <p className={styles.help}>{t("notifications.enableHelp")}</p>
      </div>
      {(prefs.notifyEnabled || permission === "denied") && permissionNotice}

      {prefs.notifyEnabled && (
        <>
          {toggle(t("notifications.inFront"), prefs.notifyInFront, (notifyInFront) => onPrefs({ notifyInFront }))}
          <div className={styles.group}>
            <span className={styles.label}>{t("notifications.scope")}</span>
            {segmented(
              prefs.notifyAll ? "all" : "some",
              [
                { value: "all", label: t("notifications.scopeAll") },
                { value: "some", label: t("notifications.scopeSome") },
              ],
              (v) => onPrefs({ notifyAll: v === "all" }),
            )}
          </div>

          {!prefs.notifyAll && (
            <>
              {nothingChosen && <p className={styles.warning}>{t("notifications.emptySelection")}</p>}
              {checklist(
                t("notifications.folders"),
                folders.map((f) => ({ id: f.uid, label: f.name })),
                prefs.notifyFolders,
                (notifyFolders) => onPrefs({ notifyFolders }),
              )}
              {checklist(
                t("notifications.feeds"),
                feeds.map((f) => ({ id: f.url, label: f.title })),
                prefs.notifyFeeds,
                (notifyFeeds) => onPrefs({ notifyFeeds }),
              )}
              {checklist(
                t("notifications.smartFolders"),
                smartFolders.map((f) => ({ id: f.uid, label: f.name })),
                prefs.notifySmartFolders,
                (notifySmartFolders) => onPrefs({ notifySmartFolders }),
              )}
              <div className={styles.group}>
                <span className={styles.label}>{t("notifications.keywords")}</span>
                {prefs.notifyKeywords.length > 0 && (
                  <div className={styles.chips}>
                    {prefs.notifyKeywords.map((k) => (
                      <span key={k} className={styles.chip}>
                        {k}
                        <button
                          type="button"
                          aria-label={t("notifications.removeKeyword", { keyword: k })}
                          title={t("notifications.removeKeyword", { keyword: k })}
                          onClick={() => onPrefs({ notifyKeywords: prefs.notifyKeywords.filter((x) => x !== k) })}
                        >
                          <X size={12} />
                        </button>
                      </span>
                    ))}
                  </div>
                )}
                <form
                  className={styles.addRow}
                  onSubmit={(e) => {
                    e.preventDefault();
                    addKeyword();
                  }}
                >
                  <input value={keyword} onChange={(e) => setKeyword(e.target.value)} placeholder={t("notifications.keywordPlaceholder")} />
                  <button type="submit" className={styles.button} disabled={!keyword.trim()}>
                    {t("notifications.add")}
                  </button>
                </form>
                <p className={styles.help}>{t("notifications.keywordsHelp")}</p>
              </div>
            </>
          )}
        </>
      )}

      <div className={styles.group}>
        {toggle(t("notifications.menuBar"), prefs.menuBar, (menuBar) => onPrefs({ menuBar }))}
        <p className={styles.help}>{t("notifications.menuBarHelp")}</p>
      </div>

      <div className={styles.group}>
        <div>
          <button type="button" className={styles.button} onClick={test}>
            {t("notifications.test")}
          </button>
        </div>
        {testResult && <p className={styles.help}>{testResult}</p>}
        <p className={styles.help}>{t("notifications.perMac")}</p>
      </div>
    </>
  );
}
