import Charts
import SwiftData
import SwiftUI

/// Entrée de menu Présentation › Statistiques de lecture
struct StatsMenuItem: View {
    @Environment(\.openWindow) private var openWindow

    var body: some View {
        Button("Statistiques de lecture") { openWindow(id: "stats") }
            .keyboardShortcut("s", modifiers: [.command, .option])
    }
}

/// Présentation › Agrandir / Réduire le texte de l'article
struct ReaderTextSizeCommands: View {
    @AppStorage(Prefs.readerFontSize) private var size = 17

    var body: some View {
        Button("Agrandir le texte") { size = min(size + 1, Prefs.readerFontSizes.upperBound) }
            .keyboardShortcut("+", modifiers: .command)
            .disabled(size >= Prefs.readerFontSizes.upperBound)
        Button("Réduire le texte") { size = max(size - 1, Prefs.readerFontSizes.lowerBound) }
            .keyboardShortcut("-", modifiers: .command)
            .disabled(size <= Prefs.readerFontSizes.lowerBound)
        Button("Taille par défaut") { size = 17 }
            .keyboardShortcut("0", modifiers: .command)
            .disabled(size == 17)
    }
}

/// Fenêtre « Statistiques de lecture »
struct StatsView: View {
    enum Period: Int, CaseIterable, Identifiable {
        case week = 7, month = 30, quarter = 90, year = 365
        var id: Int { rawValue }
        var label: String {
            switch self {
            case .week: String(localized: "7 jours")
            case .month: String(localized: "30 jours")
            case .quarter: String(localized: "3 mois")
            case .year: String(localized: "12 mois")
            }
        }
    }

    @Query private var feeds: [Feed]
    @Environment(\.modelContext) private var context
    @State private var period: Period = .month
    @State private var stats = Stats()
    @State private var hoveredDay: Date?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 22) {
                HStack {
                    Text("Statistiques de lecture")
                        .font(.title2.bold())
                    Spacer()
                    Picker("Période", selection: $period) {
                        ForEach(Period.allCases) { Text($0.label).tag($0) }
                    }
                    .pickerStyle(.segmented)
                    .labelsHidden()
                    .frame(width: 320)
                }

                // Chiffres clés
                HStack(spacing: 12) {
                    tile(String(localized: "Articles lus"), value: stats.readCount.formatted(),
                         detail: String(localized: "sur \(stats.receivedCount) reçus"))
                    tile(String(localized: "Temps de lecture"), value: duration(stats.readingSeconds),
                         detail: String(localized: "temps d'affichage"))
                    tile(String(localized: "Temps estimé"), value: duration(stats.estimatedSeconds),
                         detail: String(localized: "à 230 mots par minute"))
                    tile(String(localized: "Par jour"), value: stats.perDay.formatted(.number.precision(.fractionLength(0...1))),
                         detail: String(localized: "articles lus en moyenne"))
                }

                section(String(localized: "Articles lus par jour")) {
                    if stats.daily.allSatisfy({ $0.count == 0 }) {
                        empty
                    } else {
                        Chart {
                            ForEach(stats.daily) { day in
                                BarMark(
                                    x: .value("Jour", day.date, unit: period == .year ? .weekOfYear : .day),
                                    y: .value("Articles lus", day.count)
                                )
                                .foregroundStyle(Color.accentColor.opacity(hoveredDay == nil || sameBucket(hoveredDay!, day.date) ? 1 : 0.45))
                                .cornerRadius(3)
                            }
                            if let hoveredDay, let day = stats.daily.first(where: { sameBucket($0.date, hoveredDay) }) {
                                RuleMark(x: .value("Jour", day.date, unit: period == .year ? .weekOfYear : .day))
                                    .foregroundStyle(.clear)
                                    .annotation(position: .top, spacing: 4, overflowResolution: .init(x: .fit, y: .disabled)) {
                                        VStack(spacing: 1) {
                                            Text(day.count.formatted()).font(.callout.weight(.semibold))
                                            Text(day.date, format: period == .year
                                                 ? .dateTime.day().month(.abbreviated)
                                                 : .dateTime.weekday(.abbreviated).day().month(.abbreviated))
                                                .font(.caption)
                                                .foregroundStyle(.secondary)
                                        }
                                        .padding(.horizontal, 8)
                                        .padding(.vertical, 4)
                                        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 6))
                                    }
                            }
                        }
                        .chartXSelection(value: $hoveredDay)
                        .chartYAxis {
                            AxisMarks(position: .leading) { _ in
                                AxisGridLine().foregroundStyle(.quaternary)
                                AxisValueLabel()
                            }
                        }
                        .frame(height: 200)
                    }
                }

                HStack(alignment: .top, spacing: 16) {
                    section(String(localized: "Sources les plus lues")) {
                        ranking(stats.mostRead) { $0 == 1 ? String(localized: "1 lu") : String(localized: "\($0) lus") }
                    }
                    section(String(localized: "Sources les plus actives")) {
                        ranking(stats.mostActive) { $0 == 1 ? String(localized: "1 publié") : String(localized: "\($0) publiés") }
                    }
                }

                Text(stats.trackingSince.map {
                    String(localized: "Lectures enregistrées depuis le \($0.formatted(date: .long, time: .omitted)). Un article compte comme lu après 3 secondes d'affichage. Les articles publiés ne sont comptés que tant qu'ils sont conservés.")
                } ?? String(localized: "Aucune lecture enregistrée pour l'instant. Un article compte comme lu après 3 secondes d'affichage."))
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            .padding(24)
        }
        .frame(minWidth: 720, minHeight: 620)
        .onAppear { stats = Stats.compute(period: period, context: context, feeds: feeds) }
        .onChange(of: period) { stats = Stats.compute(period: period, context: context, feeds: feeds) }
    }

    // MARK: - Éléments

    private func tile(_ title: String, value: String, detail: String) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(title).font(.callout).foregroundStyle(.secondary)
            Text(value).font(.system(size: 26, weight: .semibold, design: .rounded)).monospacedDigit()
            Text(detail).font(.caption).foregroundStyle(.tertiary)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(14)
        .background(.quaternary.opacity(0.5), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
    }

    private func section<Content: View>(_ title: String, @ViewBuilder content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(title).font(.headline)
            content()
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    /// Classement en barres horizontales, nom et valeur écrits à côté
    private func ranking(_ rows: [Stats.Row], label: @escaping (Int) -> String) -> some View {
        Group {
            if rows.isEmpty {
                empty
            } else {
                let maxValue = Double(rows.map(\.count).max() ?? 1)
                VStack(alignment: .leading, spacing: 9) {
                    ForEach(rows) { row in
                        VStack(alignment: .leading, spacing: 3) {
                            HStack {
                                Text(row.name).lineLimit(1)
                                Spacer()
                                Text(label(row.count))
                                    .foregroundStyle(.secondary)
                                    .monospacedDigit()
                            }
                            .font(.callout)
                            GeometryReader { geo in
                                RoundedRectangle(cornerRadius: 3)
                                    .fill(Color.accentColor)
                                    .frame(width: max(4, geo.size.width * Double(row.count) / maxValue))
                            }
                            .frame(height: 6)
                        }
                        .help(Text(verbatim: "\(row.name) : \(label(row.count))"))
                    }
                }
            }
        }
    }

    private var empty: some View {
        Text("Pas encore de données sur cette période.")
            .font(.callout)
            .foregroundStyle(.secondary)
            .frame(maxWidth: .infinity, minHeight: 60)
    }

    private func sameBucket(_ a: Date, _ b: Date) -> Bool {
        Calendar.current.isDate(a, equalTo: b, toGranularity: period == .year ? .weekOfYear : .day)
    }

    private func duration(_ seconds: Double) -> String {
        let formatter = DateComponentsFormatter()
        formatter.allowedUnits = seconds >= 3600 ? [.hour, .minute] : [.minute]
        formatter.unitsStyle = .short   // « 14 min », « 1 h 5 min »
        formatter.zeroFormattingBehavior = .dropLeading
        return formatter.string(from: max(seconds, 0)) ?? "0"
    }
}

/// Calcul des statistiques sur une période
struct Stats {
    struct Day: Identifiable {
        let date: Date
        let count: Int
        var id: Date { date }
    }

    struct Row: Identifiable {
        let name: String
        let count: Int
        var id: String { name }
    }

    var readCount = 0
    var receivedCount = 0
    var readingSeconds: Double = 0
    var estimatedSeconds: Double = 0
    var perDay: Double = 0
    var daily: [Day] = []
    var mostRead: [Row] = []
    var mostActive: [Row] = []
    var trackingSince: Date?

    @MainActor
    static func compute(period: StatsView.Period, context: ModelContext, feeds: [Feed]) -> Stats {
        var s = Stats()
        let calendar = Calendar.current
        let start = calendar.startOfDay(for: Date.now.addingTimeInterval(-Double(period.rawValue - 1) * 86_400))
        let names = Dictionary(feeds.map { ($0.url, $0.displayTitle) }, uniquingKeysWith: { a, _ in a })

        let past = Date.distantPast
        let opened = (try? context.fetch(FetchDescriptor<Article>(
            predicate: #Predicate { ($0.openedAt ?? past) >= start }))) ?? []
        let published = (try? context.fetch(FetchDescriptor<Article>(
            predicate: #Predicate { $0.published >= start && !$0.isHidden }))) ?? []

        s.readCount = opened.count
        s.receivedCount = published.count
        s.readingSeconds = opened.reduce(0) { $0 + $1.readingSeconds }
        s.estimatedSeconds = opened.reduce(0) { $0 + $1.estimatedReadingSeconds }
        s.perDay = Double(opened.count) / Double(period.rawValue)

        // Par jour (par semaine sur 12 mois)
        let unit: Calendar.Component = period == .year ? .weekOfYear : .day
        let bucket: (Date) -> Date = { date in
            unit == .day ? calendar.startOfDay(for: date)
                : calendar.dateInterval(of: .weekOfYear, for: date)?.start ?? calendar.startOfDay(for: date)
        }
        var counts: [Date: Int] = [:]
        for article in opened { if let d = article.openedAt { counts[bucket(d), default: 0] += 1 } }
        var cursor = bucket(start)
        while cursor <= Date.now {
            s.daily.append(Day(date: cursor, count: counts[cursor] ?? 0))
            cursor = calendar.date(byAdding: unit, value: 1, to: cursor) ?? Date.distantFuture
        }

        func top(_ articles: [Article]) -> [Row] {
            Dictionary(grouping: articles, by: \.feedURL)
                .map { Row(name: names[$0.key] ?? URL(string: $0.key)?.host() ?? $0.key, count: $0.value.count) }
                .sorted { $0.count > $1.count }
                .prefix(8)
                .map { $0 }
        }
        s.mostRead = top(opened)
        s.mostActive = top(published)

        var first = FetchDescriptor<Article>(predicate: #Predicate { $0.openedAt != nil },
                                             sortBy: [SortDescriptor<Article>(\.openedAt)])
        first.fetchLimit = 1
        s.trackingSince = (try? context.fetch(first).first)?.openedAt
        return s
    }
}
