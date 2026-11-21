pub(crate) struct DialogLabels {
    pub import: &'static str,
    pub backup: &'static str,
    pub qr_image: &'static str,
}

pub(crate) fn for_locale(locale: &str) -> DialogLabels {
    match locale {
        "zh-TW" => DialogLabels {
            import: "驗證器匯出檔案",
            backup: "AUTHENTICATOR DESK 備份",
            qr_image: "QR 碼圖片",
        },
        "en" => DialogLabels {
            import: "Authenticator export",
            backup: "AUTHENTICATOR DESK backup",
            qr_image: "QR image",
        },
        "ja" => DialogLabels {
            import: "認証アプリのエクスポート",
            backup: "AUTHENTICATOR DESK バックアップ",
            qr_image: "QR コード画像",
        },
        _ => DialogLabels {
            import: "验证器导出文件",
            backup: "AUTHENTICATOR DESK 备份",
            qr_image: "二维码图片",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::for_locale;

    #[test]
    fn native_filters_follow_the_four_supported_locales() {
        for (locale, import, backup, qr_image) in [
            (
                "zh-CN",
                "验证器导出文件",
                "AUTHENTICATOR DESK 备份",
                "二维码图片",
            ),
            (
                "zh-TW",
                "驗證器匯出檔案",
                "AUTHENTICATOR DESK 備份",
                "QR 碼圖片",
            ),
            (
                "en",
                "Authenticator export",
                "AUTHENTICATOR DESK backup",
                "QR image",
            ),
            (
                "ja",
                "認証アプリのエクスポート",
                "AUTHENTICATOR DESK バックアップ",
                "QR コード画像",
            ),
        ] {
            let labels = for_locale(locale);
            assert_eq!(labels.import, import);
            assert_eq!(labels.backup, backup);
            assert_eq!(labels.qr_image, qr_image);
        }
    }

    #[test]
    fn unsupported_locales_use_the_default_labels() {
        for locale in ["", "fr", "../../settings", "en-US"] {
            let labels = for_locale(locale);
            assert_eq!(labels.import, for_locale("zh-CN").import);
            assert_eq!(labels.backup, for_locale("zh-CN").backup);
            assert_eq!(labels.qr_image, for_locale("zh-CN").qr_image);
        }
    }
}
