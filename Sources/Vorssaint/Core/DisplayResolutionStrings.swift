// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Vorssaint

import Foundation

/// HiDPI size control on external display rows. Same contract as the other
/// FeatureStrings structs: memberwise init in declaration order, one static
/// per language, all in this file.
struct DisplayResolutionFeatureStrings {
    let hiDPI: String
    let off: String
    let caption: String
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
        off: "Off",
        caption: "Draws this display at twice its pixel density for sharper text. A display without HiDPI modes of its own mirrors a virtual HiDPI display, which caps it at 60 Hz and turns HDR off."
    )

    static let ptBR = DisplayResolutionFeatureStrings(
        hiDPI: "HiDPI",
        off: "Desligado",
        caption: "Desenha esta tela com o dobro da densidade de pixels para um texto mais nítido. Uma tela sem modos HiDPI próprios espelha uma tela HiDPI virtual, o que a limita a 60 Hz e desliga o HDR."
    )

    static let tr = DisplayResolutionFeatureStrings(
        hiDPI: "HiDPI",
        off: "Kapalı",
        caption: "Daha keskin metin için bu ekranı piksel yoğunluğunun iki katında çizer. Kendi HiDPI modu olmayan bir ekran sanal bir HiDPI ekranını yansıtır; bu da onu 60 Hz ile sınırlar ve HDR’yi kapatır."
    )

    static let ru = DisplayResolutionFeatureStrings(
        hiDPI: "HiDPI",
        off: "Выкл.",
        caption: "Отрисовывает этот дисплей с удвоенной плотностью пикселей для более чёткого текста. Дисплей без собственных режимов HiDPI повторяет виртуальный дисплей HiDPI, что ограничивает его 60 Гц и отключает HDR."
    )

    static let es = DisplayResolutionFeatureStrings(
        hiDPI: "HiDPI",
        off: "Desactivado",
        caption: "Dibuja esta pantalla al doble de su densidad de píxeles para un texto más nítido. Una pantalla sin modos HiDPI propios duplica una pantalla HiDPI virtual, lo que la limita a 60 Hz y desactiva el HDR."
    )

    static let sk = DisplayResolutionFeatureStrings(
        hiDPI: "HiDPI",
        off: "Vypnuté",
        caption: "Vykresľuje tento displej s dvojnásobnou hustotou pixelov pre ostrejší text. Displej bez vlastných režimov HiDPI zrkadlí virtuálny displej HiDPI, čo ho obmedzí na 60 Hz a vypne HDR."
    )

    static let de = DisplayResolutionFeatureStrings(
        hiDPI: "HiDPI",
        off: "Aus",
        caption: "Stellt diesen Bildschirm mit doppelter Pixeldichte dar, für schärferen Text. Ein Bildschirm ohne eigene HiDPI-Modi spiegelt einen virtuellen HiDPI-Bildschirm, was ihn auf 60 Hz begrenzt und HDR ausschaltet."
    )

    static let fr = DisplayResolutionFeatureStrings(
        hiDPI: "HiDPI",
        off: "Désactivé",
        caption: "Affiche cet écran avec le double de sa densité de pixels pour un texte plus net. Un écran sans mode HiDPI propre recopie un écran HiDPI virtuel, ce qui le limite à 60 Hz et désactive le HDR."
    )

    static let it = DisplayResolutionFeatureStrings(
        hiDPI: "HiDPI",
        off: "Disattivato",
        caption: "Disegna questo schermo al doppio della sua densità di pixel per un testo più nitido. Uno schermo senza modalità HiDPI proprie duplica uno schermo HiDPI virtuale, che lo limita a 60 Hz e disattiva l’HDR."
    )

    static let ja = DisplayResolutionFeatureStrings(
        hiDPI: "HiDPI",
        off: "オフ",
        caption: "文字をくっきり表示するため、このディスプレイを2倍のピクセル密度で描画します。独自のHiDPIモードがないディスプレイは仮想HiDPIディスプレイをミラーリングするため、60 Hzに制限され、HDRはオフになります。"
    )

    static let ko = DisplayResolutionFeatureStrings(
        hiDPI: "HiDPI",
        off: "끔",
        caption: "더 선명한 텍스트를 위해 이 디스플레이를 두 배의 픽셀 밀도로 그립니다. 자체 HiDPI 모드가 없는 디스플레이는 가상 HiDPI 디스플레이를 미러링하므로 60Hz로 제한되고 HDR이 꺼집니다."
    )

    static let uk = DisplayResolutionFeatureStrings(
        hiDPI: "HiDPI",
        off: "Вимк.",
        caption: "Відображає цей дисплей із подвійною щільністю пікселів для чіткішого тексту. Дисплей без власних режимів HiDPI віддзеркалює віртуальний дисплей HiDPI, що обмежує його до 60 Гц і вимикає HDR."
    )

    static let zhHans = DisplayResolutionFeatureStrings(
        hiDPI: "HiDPI",
        off: "关闭",
        caption: "以两倍像素密度绘制此显示器，让文字更清晰。没有自身 HiDPI 模式的显示器会镜像一个虚拟 HiDPI 显示器，这会将其限制为 60 Hz 并关闭 HDR。"
    )

    static let zhTW = DisplayResolutionFeatureStrings(
        hiDPI: "HiDPI",
        off: "關閉",
        caption: "以兩倍像素密度繪製此顯示器，讓文字更清晰。沒有自身 HiDPI 模式的顯示器會鏡像輸出一個虛擬 HiDPI 顯示器，這會將其限制為 60 Hz 並關閉 HDR。"
    )

    static let zhHK = DisplayResolutionFeatureStrings(
        hiDPI: "HiDPI",
        off: "關閉",
        caption: "以兩倍像素密度繪製此顯示器，令文字更清晰。沒有自身 HiDPI 模式的顯示器會鏡像輸出一個虛擬 HiDPI 顯示器，這會將其限制為 60 Hz 並關閉 HDR。"
    )
}
