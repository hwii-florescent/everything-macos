// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Vorssaint

import Foundation

/// HiDPI size control on built-in and external display rows. Same contract as the other
/// FeatureStrings structs: memberwise init in declaration order, one static
/// per language, all in this file.
struct DisplayResolutionFeatureStrings {
    let hiDPI: String
    let off: String
    let caption: String
    let defaultMode: String
    let builtInCaption: String
    let scaling: String
    let sizeHint: String
    let largerText: String
    let moreSpace: String
    let hiDPIOff: String
    let statusUnavailable: String

    func status(for hiDPI: Bool?) -> String {
        hiDPI.map { $0 ? self.hiDPI : hiDPIOff } ?? statusUnavailable
    }
}

extension FeatureStrings {
    static func displayResolution(_ language: AppLanguage) -> DisplayResolutionFeatureStrings {
        switch language {
        case .enUS: return .enUS
        case .ptBR: return .ptBR
        case .tr: return .tr
        case .ru: return .ru
        case .es: return .es
        case .sk: return .sk
        case .de: return .de
        case .fr: return .fr
        case .it: return .it
        case .ja: return .ja
        case .ko: return .ko
        case .uk: return .uk
        case .zhHans: return .zhHans
        case .zhTW: return .zhTW
        case .zhHK: return .zhHK
        }
    }
}

extension DisplayResolutionFeatureStrings {
    static let enUS = DisplayResolutionFeatureStrings(
        hiDPI: "HiDPI",
        off: "Use display default",
        caption: "Draws this display at twice its pixel density for sharper text. A display without HiDPI modes of its own mirrors a virtual HiDPI display, which caps it at 60 Hz and turns HDR off.",
        defaultMode: "Default",
        builtInCaption: "Changes the built-in display’s Retina scaling using macOS-supported sizes. Default restores the system’s recommended size.",
        scaling: "Display scaling",
        sizeHint: "Smaller numbers give larger text; larger numbers give more space.",
        largerText: "Larger text",
        moreSpace: "More space",
        hiDPIOff: "HiDPI off",
        statusUnavailable: "Status unavailable"
    )

    static let ptBR = DisplayResolutionFeatureStrings(
        hiDPI: "HiDPI",
        off: "Usar padrão da tela",
        caption: "Desenha esta tela com o dobro da densidade de pixels para um texto mais nítido. Uma tela sem modos HiDPI próprios espelha uma tela HiDPI virtual, o que a limita a 60 Hz e desliga o HDR.",
        defaultMode: "Padrão",
        builtInCaption: "Altera a escala Retina da tela integrada usando tamanhos compatíveis com o macOS. Padrão restaura o tamanho recomendado pelo sistema.",
        scaling: "Escala da tela",
        sizeHint: "Números menores ampliam o texto; números maiores oferecem mais espaço.",
        largerText: "Texto maior",
        moreSpace: "Mais espaço",
        hiDPIOff: "HiDPI desligado",
        statusUnavailable: "Estado indisponível"
    )

    static let tr = DisplayResolutionFeatureStrings(
        hiDPI: "HiDPI",
        off: "Ekran varsayılanını kullan",
        caption: "Daha keskin metin için bu ekranı piksel yoğunluğunun iki katında çizer. Kendi HiDPI modu olmayan bir ekran sanal bir HiDPI ekranını yansıtır; bu da onu 60 Hz ile sınırlar ve HDR’yi kapatır.",
        defaultMode: "Varsayılan",
        builtInCaption: "Yerleşik ekranın Retina ölçeklemesini macOS’un desteklediği boyutlarla değiştirir. Varsayılan, sistemin önerdiği boyutu geri yükler.",
        scaling: "Ekran ölçekleme",
        sizeHint: "Küçük sayılar metni büyütür; büyük sayılar daha fazla alan sağlar.",
        largerText: "Daha büyük metin",
        moreSpace: "Daha fazla alan",
        hiDPIOff: "HiDPI kapalı",
        statusUnavailable: "Durum kullanılamıyor"
    )

    static let ru = DisplayResolutionFeatureStrings(
        hiDPI: "HiDPI",
        off: "Стандартный режим дисплея",
        caption: "Отрисовывает этот дисплей с удвоенной плотностью пикселей для более чёткого текста. Дисплей без собственных режимов HiDPI повторяет виртуальный дисплей HiDPI, что ограничивает его 60 Гц и отключает HDR.",
        defaultMode: "По умолчанию",
        builtInCaption: "Меняет масштаб Retina встроенного дисплея, используя размеры, поддерживаемые macOS. «По умолчанию» восстанавливает рекомендуемый системой размер.",
        scaling: "Масштаб экрана",
        sizeHint: "Меньшие числа увеличивают текст, большие дают больше места.",
        largerText: "Крупнее текст",
        moreSpace: "Больше места",
        hiDPIOff: "HiDPI выкл.",
        statusUnavailable: "Статус недоступен"
    )

    static let es = DisplayResolutionFeatureStrings(
        hiDPI: "HiDPI",
        off: "Usar valor predeterminado de pantalla",
        caption: "Dibuja esta pantalla al doble de su densidad de píxeles para un texto más nítido. Una pantalla sin modos HiDPI propios duplica una pantalla HiDPI virtual, lo que la limita a 60 Hz y desactiva el HDR.",
        defaultMode: "Predeterminado",
        builtInCaption: "Cambia la escala Retina de la pantalla integrada con tamaños compatibles con macOS. Predeterminado restaura el tamaño recomendado por el sistema.",
        scaling: "Escala de pantalla",
        sizeHint: "Los números menores amplían el texto; los mayores dan más espacio.",
        largerText: "Texto más grande",
        moreSpace: "Más espacio",
        hiDPIOff: "HiDPI desactivado",
        statusUnavailable: "Estado no disponible"
    )

    static let sk = DisplayResolutionFeatureStrings(
        hiDPI: "HiDPI",
        off: "Použiť predvolený režim displeja",
        caption: "Vykresľuje tento displej s dvojnásobnou hustotou pixelov pre ostrejší text. Displej bez vlastných režimov HiDPI zrkadlí virtuálny displej HiDPI, čo ho obmedzí na 60 Hz a vypne HDR.",
        defaultMode: "Predvolené",
        builtInCaption: "Mení škálovanie Retina vstavaného displeja pomocou veľkostí podporovaných systémom macOS. Predvolené obnoví veľkosť odporúčanú systémom.",
        scaling: "Mierka displeja",
        sizeHint: "Menšie čísla zväčšia text; väčšie poskytnú viac priestoru.",
        largerText: "Väčší text",
        moreSpace: "Viac priestoru",
        hiDPIOff: "HiDPI vypnuté",
        statusUnavailable: "Stav nie je dostupný"
    )

    static let de = DisplayResolutionFeatureStrings(
        hiDPI: "HiDPI",
        off: "Bildschirmstandard verwenden",
        caption: "Stellt diesen Bildschirm mit doppelter Pixeldichte dar, für schärferen Text. Ein Bildschirm ohne eigene HiDPI-Modi spiegelt einen virtuellen HiDPI-Bildschirm, was ihn auf 60 Hz begrenzt und HDR ausschaltet.",
        defaultMode: "Standard",
        builtInCaption: "Ändert die Retina-Skalierung des integrierten Bildschirms mit von macOS unterstützten Größen. Standard stellt die vom System empfohlene Größe wieder her.",
        scaling: "Bildschirmskalierung",
        sizeHint: "Kleinere Zahlen vergrößern Text; größere schaffen mehr Platz.",
        largerText: "Größerer Text",
        moreSpace: "Mehr Platz",
        hiDPIOff: "HiDPI aus",
        statusUnavailable: "Status nicht verfügbar"
    )

    static let fr = DisplayResolutionFeatureStrings(
        hiDPI: "HiDPI",
        off: "Utiliser le réglage par défaut de l’écran",
        caption: "Affiche cet écran avec le double de sa densité de pixels pour un texte plus net. Un écran sans mode HiDPI propre recopie un écran HiDPI virtuel, ce qui le limite à 60 Hz et désactive le HDR.",
        defaultMode: "Par défaut",
        builtInCaption: "Modifie la mise à l’échelle Retina de l’écran intégré avec les tailles prises en charge par macOS. Par défaut rétablit la taille recommandée par le système.",
        scaling: "Échelle de l’écran",
        sizeHint: "Les petits nombres agrandissent le texte ; les grands offrent plus d’espace.",
        largerText: "Texte plus grand",
        moreSpace: "Plus d’espace",
        hiDPIOff: "HiDPI désactivé",
        statusUnavailable: "État indisponible"
    )

    static let it = DisplayResolutionFeatureStrings(
        hiDPI: "HiDPI",
        off: "Usa impostazione predefinita dello schermo",
        caption: "Disegna questo schermo al doppio della sua densità di pixel per un testo più nitido. Uno schermo senza modalità HiDPI proprie duplica uno schermo HiDPI virtuale, che lo limita a 60 Hz e disattiva l’HDR.",
        defaultMode: "Predefinita",
        builtInCaption: "Modifica il ridimensionamento Retina dello schermo integrato con dimensioni supportate da macOS. Predefinita ripristina la dimensione consigliata dal sistema.",
        scaling: "Scala dello schermo",
        sizeHint: "Numeri più piccoli ingrandiscono il testo; numeri più grandi offrono più spazio.",
        largerText: "Testo più grande",
        moreSpace: "Più spazio",
        hiDPIOff: "HiDPI disattivato",
        statusUnavailable: "Stato non disponibile"
    )

    static let ja = DisplayResolutionFeatureStrings(
        hiDPI: "HiDPI",
        off: "ディスプレイのデフォルトを使用",
        caption: "文字をくっきり表示するため、このディスプレイを2倍のピクセル密度で描画します。独自のHiDPIモードがないディスプレイは仮想HiDPIディスプレイをミラーリングするため、60 Hzに制限され、HDRはオフになります。",
        defaultMode: "デフォルト",
        builtInCaption: "macOSが対応するサイズで内蔵ディスプレイのRetinaスケーリングを変更します。「デフォルト」でシステム推奨のサイズに戻します。",
        scaling: "画面の拡大率",
        sizeHint: "小さい数値は文字を大きくし、大きい数値は作業領域を広げます。",
        largerText: "文字を大きく",
        moreSpace: "作業領域を広く",
        hiDPIOff: "HiDPIオフ",
        statusUnavailable: "状態を取得できません"
    )

    static let ko = DisplayResolutionFeatureStrings(
        hiDPI: "HiDPI",
        off: "디스플레이 기본값 사용",
        caption: "더 선명한 텍스트를 위해 이 디스플레이를 두 배의 픽셀 밀도로 그립니다. 자체 HiDPI 모드가 없는 디스플레이는 가상 HiDPI 디스플레이를 미러링하므로 60Hz로 제한되고 HDR이 꺼집니다.",
        defaultMode: "기본값",
        builtInCaption: "macOS에서 지원하는 크기로 내장 디스플레이의 Retina 배율을 변경합니다. 기본값은 시스템 권장 크기로 복원합니다.",
        scaling: "디스플레이 배율",
        sizeHint: "숫자가 작으면 글자가 커지고, 숫자가 크면 작업 공간이 넓어집니다.",
        largerText: "더 큰 텍스트",
        moreSpace: "더 넓은 공간",
        hiDPIOff: "HiDPI 끔",
        statusUnavailable: "상태를 확인할 수 없음"
    )

    static let uk = DisplayResolutionFeatureStrings(
        hiDPI: "HiDPI",
        off: "Стандартний режим дисплея",
        caption: "Відображає цей дисплей із подвійною щільністю пікселів для чіткішого тексту. Дисплей без власних режимів HiDPI віддзеркалює віртуальний дисплей HiDPI, що обмежує його до 60 Гц і вимикає HDR.",
        defaultMode: "Типовий",
        builtInCaption: "Змінює масштаб Retina вбудованого дисплея, використовуючи розміри, підтримувані macOS. «Типовий» відновлює рекомендований системою розмір.",
        scaling: "Масштаб екрана",
        sizeHint: "Менші числа збільшують текст, більші дають більше місця.",
        largerText: "Більший текст",
        moreSpace: "Більше місця",
        hiDPIOff: "HiDPI вимк.",
        statusUnavailable: "Статус недоступний"
    )

    static let zhHans = DisplayResolutionFeatureStrings(
        hiDPI: "HiDPI",
        off: "使用显示器默认设置",
        caption: "以两倍像素密度绘制此显示器，让文字更清晰。没有自身 HiDPI 模式的显示器会镜像一个虚拟 HiDPI 显示器，这会将其限制为 60 Hz 并关闭 HDR。",
        defaultMode: "默认",
        builtInCaption: "使用 macOS 支持的尺寸更改内建显示器的 Retina 缩放比例。“默认”会恢复系统推荐的尺寸。",
        scaling: "显示缩放",
        sizeHint: "数值越小，文字越大；数值越大，工作空间越多。",
        largerText: "更大文字",
        moreSpace: "更多空间",
        hiDPIOff: "HiDPI 已关闭",
        statusUnavailable: "状态不可用"
    )

    static let zhTW = DisplayResolutionFeatureStrings(
        hiDPI: "HiDPI",
        off: "使用顯示器預設設定",
        caption: "以兩倍像素密度繪製此顯示器，讓文字更清晰。沒有自身 HiDPI 模式的顯示器會鏡像輸出一個虛擬 HiDPI 顯示器，這會將其限制為 60 Hz 並關閉 HDR。",
        defaultMode: "預設",
        builtInCaption: "使用 macOS 支援的尺寸更改內建顯示器的 Retina 縮放比例。「預設」會還原系統建議的尺寸。",
        scaling: "顯示縮放",
        sizeHint: "數值越小，文字越大；數值越大，工作空間越多。",
        largerText: "較大文字",
        moreSpace: "更多空間",
        hiDPIOff: "HiDPI 已關閉",
        statusUnavailable: "無法取得狀態"
    )

    static let zhHK = DisplayResolutionFeatureStrings(
        hiDPI: "HiDPI",
        off: "使用顯示器預設設定",
        caption: "以兩倍像素密度繪製此顯示器，令文字更清晰。沒有自身 HiDPI 模式的顯示器會鏡像輸出一個虛擬 HiDPI 顯示器，這會將其限制為 60 Hz 並關閉 HDR。",
        defaultMode: "預設",
        builtInCaption: "使用 macOS 支援的尺寸更改內置顯示器的 Retina 縮放比例。「預設」會還原系統建議的尺寸。",
        scaling: "顯示縮放",
        sizeHint: "數值越小，文字越大；數值越大，工作空間越多。",
        largerText: "較大文字",
        moreSpace: "更多空間",
        hiDPIOff: "HiDPI 已關閉",
        statusUnavailable: "無法取得狀態"
    )
}
