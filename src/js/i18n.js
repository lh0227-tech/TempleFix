/* ===== TempleFix 界面多语言基础 ===== */
(function (global) {
  "use strict";

  const dictionaries = {
    "zh-CN": {
      app_name: "太阳穴 TempleFix",
      skip: "稍后设置",
      back: "上一步",
      next: "下一步",
      welcome_title: "欢迎使用太阳穴",
      welcome_lead: "截图、识别文字，再把文字交给你选择的 AI 服务翻译。",
      welcome_local: "太阳穴负责截图和本地 OCR，但不提供大语言模型服务。完成 AI 翻译，需要配置你自己的 API Key。",
      welcome_key: "API Key 是你的 AI 服务凭证，只保存在这台电脑，只会发送给你选择的服务商。太阳穴不出售、也不代充模型服务。",
      ui_language: "界面语言",
      native_language: "翻译成",
      service_title: "配置 AI 翻译服务",
      service_lead: "推荐先填好并测试连接；也可以稍后设置，只使用文字提取。",
      provider: "服务类型",
      base_url: "接口地址",
      model: "模型名称",
      api_key: "API Key（密钥）",
      api_key_placeholder: "粘贴你的密钥",
      test_connection: "测试连接",
      testing: "正在连接…",
      test_ok: "连接成功",
      required_fields: "请先填写接口地址、模型名称和 API Key",
      ocr_title: "增强 OCR 是可选的",
      ocr_lead: "不安装也能使用 Windows 自带 OCR。增强组件适合中、英、日和多种拉丁文字，图片识别始终在本机完成。",
      ocr_installed: "增强 OCR 已安装",
      ocr_not_installed: "当前未安装，可稍后在首选项中一键安装",
      open_preferences: "打开首选项管理",
      finish_title: "准备好了",
      finish_lead: "太阳穴会驻留在系统托盘，不会一直占着桌面。",
      finish_hotkey: "按 Alt+Z，框选屏幕内容即可开始。",
      finish_service_ready: "AI 翻译服务已经配置好。",
      finish_service_later: "你选择了稍后配置；需要翻译时会给出正常引导，不会连续报错。",
      finish: "完成并开始使用",
      save_failed: "保存失败",
    },
    en: {
      app_name: "TempleFix",
      skip: "Set up later",
      back: "Back",
      next: "Next",
      welcome_title: "Welcome to TempleFix",
      welcome_lead: "Capture an area, extract its text, then translate it with the AI service you choose.",
      welcome_local: "TempleFix handles screenshots and local OCR, but it does not provide a language-model service. AI translation requires your own API key.",
      welcome_key: "Your API key is your credential. It stays on this computer and is sent only to the provider you choose. TempleFix does not sell or top up model services.",
      ui_language: "Interface language",
      native_language: "Translate into",
      service_title: "Set up AI translation",
      service_lead: "We recommend testing the connection now. You can also set it up later and use text extraction only.",
      provider: "Service type",
      base_url: "Base URL",
      model: "Model",
      api_key: "API key",
      api_key_placeholder: "Paste your key",
      test_connection: "Test connection",
      testing: "Connecting…",
      test_ok: "Connection successful",
      required_fields: "Enter the Base URL, model, and API key first",
      ocr_title: "Enhanced OCR is optional",
      ocr_lead: "Windows OCR works without it. The enhanced component supports Chinese, English, Japanese, and many Latin-script languages. Image recognition stays on your computer.",
      ocr_installed: "Enhanced OCR is installed",
      ocr_not_installed: "Not installed. You can install it later from Preferences.",
      open_preferences: "Manage in Preferences",
      finish_title: "You are ready",
      finish_lead: "TempleFix stays in the system tray instead of occupying your desktop.",
      finish_hotkey: "Press Alt+Z and select an area to begin.",
      finish_service_ready: "AI translation is configured.",
      finish_service_later: "You chose to configure it later. TempleFix will show a helpful setup choice instead of repeated errors.",
      finish: "Finish and start",
      save_failed: "Could not save",
    },
    ja: {
      app_name: "TempleFix",
      skip: "後で設定",
      back: "戻る",
      next: "次へ",
      welcome_title: "TempleFixへようこそ",
      welcome_lead: "範囲を撮影して文字を認識し、選択したAIサービスで翻訳します。",
      welcome_local: "TempleFixはスクリーンショットとローカルOCRを担当しますが、言語モデルのサービスは提供しません。AI翻訳にはご自身のAPI Keyが必要です。",
      welcome_key: "API KeyはこのPCだけに保存され、選択したサービスにのみ送信されます。TempleFixがモデルサービスを販売・チャージすることはありません。",
      ui_language: "表示言語",
      native_language: "翻訳先",
      service_title: "AI翻訳サービスを設定",
      service_lead: "今ここで接続テストを行うことをおすすめします。後で設定して文字抽出だけを使うこともできます。",
      provider: "サービス種類",
      base_url: "API URL",
      model: "モデル名",
      api_key: "API Key",
      api_key_placeholder: "キーを貼り付け",
      test_connection: "接続テスト",
      testing: "接続中…",
      test_ok: "接続できました",
      required_fields: "API URL、モデル名、API Keyを入力してください",
      ocr_title: "拡張OCRは任意です",
      ocr_lead: "未導入でもWindows OCRを使えます。拡張版は中国語・英語・日本語などに対応し、画像認識はPC内だけで行われます。",
      ocr_installed: "拡張OCRはインストール済みです",
      ocr_not_installed: "未インストールです。後で設定画面から導入できます。",
      open_preferences: "設定画面で管理",
      finish_title: "準備完了です",
      finish_lead: "TempleFixはデスクトップを占有せず、システムトレイに常駐します。",
      finish_hotkey: "Alt+Zを押して画面範囲を選択してください。",
      finish_service_ready: "AI翻訳サービスを設定しました。",
      finish_service_later: "後で設定することを選びました。翻訳時にはエラーではなく設定案内を表示します。",
      finish: "完了して開始",
      save_failed: "保存できませんでした",
    },
    fr: {
      app_name: "TempleFix",
      skip: "Configurer plus tard",
      back: "Retour",
      next: "Suivant",
      welcome_title: "Bienvenue dans TempleFix",
      welcome_lead: "Capturez une zone, extrayez le texte, puis traduisez-le avec le service d’IA de votre choix.",
      welcome_local: "TempleFix gère les captures et l’OCR local, mais ne fournit pas de modèle de langage. La traduction nécessite votre propre clé API.",
      welcome_key: "La clé API reste sur cet ordinateur et n’est envoyée qu’au fournisseur choisi. TempleFix ne vend ni ne recharge de service de modèle.",
      ui_language: "Langue de l’interface",
      native_language: "Traduire vers",
      service_title: "Configurer la traduction IA",
      service_lead: "Nous conseillons de tester la connexion maintenant. Vous pouvez aussi le faire plus tard et utiliser seulement l’extraction de texte.",
      provider: "Type de service",
      base_url: "Adresse API",
      model: "Modèle",
      api_key: "Clé API",
      api_key_placeholder: "Collez votre clé",
      test_connection: "Tester la connexion",
      testing: "Connexion…",
      test_ok: "Connexion réussie",
      required_fields: "Renseignez l’adresse API, le modèle et la clé API",
      ocr_title: "L’OCR amélioré est facultatif",
      ocr_lead: "L’OCR Windows fonctionne sans lui. Le composant amélioré prend en charge plusieurs langues et traite les images uniquement sur cet ordinateur.",
      ocr_installed: "L’OCR amélioré est installé",
      ocr_not_installed: "Non installé. Vous pourrez l’installer plus tard dans les préférences.",
      open_preferences: "Gérer dans les préférences",
      finish_title: "Tout est prêt",
      finish_lead: "TempleFix reste dans la zone de notification sans encombrer le bureau.",
      finish_hotkey: "Appuyez sur Alt+Z puis sélectionnez une zone.",
      finish_service_ready: "La traduction IA est configurée.",
      finish_service_later: "Vous avez choisi de la configurer plus tard. Une aide claire remplacera les erreurs répétées.",
      finish: "Terminer et commencer",
      save_failed: "Échec de l’enregistrement",
    },
    de: {
      app_name: "TempleFix",
      skip: "Später einrichten",
      back: "Zurück",
      next: "Weiter",
      welcome_title: "Willkommen bei TempleFix",
      welcome_lead: "Bereich aufnehmen, Text erkennen und mit dem gewählten KI-Dienst übersetzen.",
      welcome_local: "TempleFix übernimmt Screenshot und lokale OCR, stellt aber keinen Sprachmodelldienst bereit. Für KI-Übersetzungen benötigen Sie einen eigenen API-Schlüssel.",
      welcome_key: "Der API-Schlüssel bleibt auf diesem Computer und wird nur an den gewählten Anbieter gesendet. TempleFix verkauft oder bezahlt keine Modelldienste.",
      ui_language: "Oberflächensprache",
      native_language: "Übersetzen nach",
      service_title: "KI-Übersetzung einrichten",
      service_lead: "Wir empfehlen, die Verbindung jetzt zu testen. Sie können später einrichten und zunächst nur Text extrahieren.",
      provider: "Diensttyp",
      base_url: "API-Adresse",
      model: "Modell",
      api_key: "API-Schlüssel",
      api_key_placeholder: "Schlüssel einfügen",
      test_connection: "Verbindung testen",
      testing: "Verbindung wird hergestellt…",
      test_ok: "Verbindung erfolgreich",
      required_fields: "Bitte API-Adresse, Modell und API-Schlüssel eingeben",
      ocr_title: "Erweiterte OCR ist optional",
      ocr_lead: "Windows OCR funktioniert auch ohne sie. Die Erweiterung unterstützt mehrere Sprachen und verarbeitet Bilder ausschließlich lokal.",
      ocr_installed: "Erweiterte OCR ist installiert",
      ocr_not_installed: "Nicht installiert. Sie kann später in den Einstellungen installiert werden.",
      open_preferences: "In Einstellungen verwalten",
      finish_title: "Alles bereit",
      finish_lead: "TempleFix bleibt im Infobereich und belegt nicht dauerhaft den Desktop.",
      finish_hotkey: "Alt+Z drücken und einen Bildschirmbereich auswählen.",
      finish_service_ready: "Die KI-Übersetzung ist eingerichtet.",
      finish_service_later: "Sie richten sie später ein. Beim Übersetzen erscheint eine klare Auswahl statt wiederholter Fehler.",
      finish: "Fertigstellen und starten",
      save_failed: "Speichern fehlgeschlagen",
    },
    es: {
      app_name: "TempleFix",
      skip: "Configurar más tarde",
      back: "Atrás",
      next: "Siguiente",
      welcome_title: "Te damos la bienvenida a TempleFix",
      welcome_lead: "Captura un área, extrae el texto y tradúcelo con el servicio de IA que elijas.",
      welcome_local: "TempleFix realiza la captura y el OCR local, pero no proporciona un modelo de lenguaje. La traducción requiere tu propia clave de API.",
      welcome_key: "La clave permanece en este equipo y solo se envía al proveedor elegido. TempleFix no vende ni recarga servicios de modelos.",
      ui_language: "Idioma de la interfaz",
      native_language: "Traducir a",
      service_title: "Configura la traducción con IA",
      service_lead: "Recomendamos probar la conexión ahora. También puedes configurarla después y usar solo la extracción de texto.",
      provider: "Tipo de servicio",
      base_url: "Dirección de API",
      model: "Modelo",
      api_key: "Clave de API",
      api_key_placeholder: "Pega tu clave",
      test_connection: "Probar conexión",
      testing: "Conectando…",
      test_ok: "Conexión correcta",
      required_fields: "Introduce la dirección, el modelo y la clave de API",
      ocr_title: "El OCR mejorado es opcional",
      ocr_lead: "Windows OCR funciona sin él. El componente mejorado admite varios idiomas y procesa las imágenes únicamente en este equipo.",
      ocr_installed: "El OCR mejorado está instalado",
      ocr_not_installed: "No está instalado. Puedes instalarlo después en Preferencias.",
      open_preferences: "Administrar en Preferencias",
      finish_title: "Todo listo",
      finish_lead: "TempleFix permanece en la bandeja del sistema sin ocupar el escritorio.",
      finish_hotkey: "Pulsa Alt+Z y selecciona un área de la pantalla.",
      finish_service_ready: "La traducción con IA está configurada.",
      finish_service_later: "Has elegido configurarla después. Verás una guía clara en lugar de errores repetidos.",
      finish: "Terminar y empezar",
      save_failed: "No se pudo guardar",
    },
    "pt-BR": {
      app_name: "TempleFix",
      skip: "Configurar depois",
      back: "Voltar",
      next: "Avançar",
      welcome_title: "Boas-vindas ao TempleFix",
      welcome_lead: "Capture uma área, extraia o texto e traduza com o serviço de IA que você escolher.",
      welcome_local: "O TempleFix cuida da captura e do OCR local, mas não fornece um modelo de linguagem. A tradução exige sua própria chave de API.",
      welcome_key: "A chave fica neste computador e só é enviada ao provedor escolhido. O TempleFix não vende nem recarrega serviços de modelos.",
      ui_language: "Idioma da interface",
      native_language: "Traduzir para",
      service_title: "Configure a tradução por IA",
      service_lead: "Recomendamos testar a conexão agora. Você também pode configurar depois e usar apenas a extração de texto.",
      provider: "Tipo de serviço",
      base_url: "Endereço da API",
      model: "Modelo",
      api_key: "Chave de API",
      api_key_placeholder: "Cole sua chave",
      test_connection: "Testar conexão",
      testing: "Conectando…",
      test_ok: "Conexão bem-sucedida",
      required_fields: "Preencha o endereço, o modelo e a chave da API",
      ocr_title: "O OCR aprimorado é opcional",
      ocr_lead: "O OCR do Windows funciona sem ele. O componente aprimorado aceita vários idiomas e processa as imagens somente neste computador.",
      ocr_installed: "O OCR aprimorado está instalado",
      ocr_not_installed: "Não instalado. Você pode instalar depois nas Preferências.",
      open_preferences: "Gerenciar nas Preferências",
      finish_title: "Tudo pronto",
      finish_lead: "O TempleFix fica na bandeja do sistema sem ocupar a área de trabalho.",
      finish_hotkey: "Pressione Alt+Z e selecione uma área da tela.",
      finish_service_ready: "A tradução por IA está configurada.",
      finish_service_later: "Você escolheu configurar depois. Uma orientação clara aparecerá no lugar de erros repetidos.",
      finish: "Concluir e começar",
      save_failed: "Não foi possível salvar",
    },
  };

  const aliases = {
    "zh": "zh-CN", "zh-cn": "zh-CN", "zh-hans": "zh-CN", "简体中文": "zh-CN",
    "en-us": "en", "en-gb": "en", "english": "en",
    "ja-jp": "ja", "日本語": "ja",
    "fr-fr": "fr", "français": "fr",
    "de-de": "de", "deutsch": "de",
    "es-es": "es", "español": "es",
    "pt": "pt-BR", "pt-br": "pt-BR", "português": "pt-BR",
  };
  let language = "en";

  function normalize(value) {
    const raw = String(value || "").trim();
    if (dictionaries[raw]) return raw;
    const lower = raw.toLowerCase();
    if (aliases[lower]) return aliases[lower];
    const base = lower.split("-")[0];
    return dictionaries[base] ? base : "";
  }

  function detect(config) {
    return normalize(config && config.ui_language) ||
      normalize(global.navigator && global.navigator.language) ||
      normalize(config && config.native_lang) || "en";
  }

  function setLanguage(value) {
    language = normalize(value) || "en";
    document.documentElement.lang = language;
    return language;
  }

  function t(key, variables) {
    const value = (dictionaries[language] && dictionaries[language][key]) ||
      dictionaries.en[key] || key;
    return String(value).replace(/\{([a-zA-Z0-9_]+)\}/g, (match, name) => {
      return variables && Object.prototype.hasOwnProperty.call(variables, name)
        ? String(variables[name])
        : match;
    });
  }

  function extend(extraDictionaries) {
    Object.entries(extraDictionaries || {}).forEach(([locale, values]) => {
      const normalized = normalize(locale) || locale;
      if (!dictionaries[normalized]) dictionaries[normalized] = {};
      Object.assign(dictionaries[normalized], values || {});
    });
  }

  function hasTranslation(value, key) {
    const normalized = normalize(value);
    return Boolean(normalized && Object.prototype.hasOwnProperty.call(dictionaries[normalized], key));
  }

  function apply(root) {
    const scope = root || document;
    scope.querySelectorAll("[data-i18n]").forEach((element) => {
      element.textContent = t(element.dataset.i18n);
    });
    scope.querySelectorAll("[data-i18n-placeholder]").forEach((element) => {
      element.placeholder = t(element.dataset.i18nPlaceholder);
    });
    scope.querySelectorAll("[data-i18n-title]").forEach((element) => {
      element.title = t(element.dataset.i18nTitle);
    });
  }

  global.TFI18n = { apply, detect, extend, hasTranslation, normalize, setLanguage, t };
})(window);
