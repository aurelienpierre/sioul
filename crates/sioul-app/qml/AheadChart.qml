// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The budgets page's one chart (research/budgets.md §4.1): an account's
// balance ahead as a step line, from today to a few days past its next
// income. Zero dashed, what the account keeps (its floor) plain, the incomes
// and the large payments marked with their names, a doubtful income's
// absence as a lighter second line; never red. The sentences above it say
// everything it shows: it is never needed to understand them.

import QtQuick

Canvas {
    id: chart

    required property var theme
    // `moneyview::ChartView`: {name, floor_label, chart: {points, without, floor_cents, marks, low_cents, high_cents, from, to}}.
    property var shown: null

    implicitHeight: 150
    Accessible.role: Accessible.Chart
    Accessible.name: chart.shown ? chart.shown.name : ""
    onShownChanged: chart.requestPaint()
    onWidthChanged: chart.requestPaint()
    onHeightChanged: chart.requestPaint()

    // A day as the chart's points write it ("2026-11-28"), in days.
    function dayOf(text) {
        return Math.round(new Date(text + "T12:00:00").getTime() / 86400000)
    }

    onPaint: {
        const ctx = chart.getContext("2d")
        ctx.reset()
        if (!chart.shown || chart.shown.chart.points.length < 2)
            return
        const c = chart.shown.chart
        const first = chart.dayOf(c.from)
        const span = Math.max(1, chart.dayOf(c.to) - first)
        const pad = Math.max(1, (c.high_cents - c.low_cents) * 0.12)
        const low = c.low_cents - pad
        const high = c.high_cents + pad
        const top = 14
        const bottom = chart.height - 16
        const x = text => 4 + (chart.dayOf(text) - first) / span * (chart.width - 8)
        const y = cents => bottom - (cents - low) / Math.max(1, high - low) * (bottom - top)
        const line = (points, alpha, width) => {
            if (points.length === 0)
                return
            ctx.globalAlpha = alpha
            ctx.strokeStyle = chart.theme.accent
            ctx.lineWidth = width
            ctx.beginPath()
            ctx.moveTo(x(points[0].day), y(points[0].cents))
            for (let i = 1; i < points.length; ++i) {
                ctx.lineTo(x(points[i].day), y(points[i - 1].cents))
                ctx.lineTo(x(points[i].day), y(points[i].cents))
            }
            ctx.stroke()
            ctx.globalAlpha = 1
        }
        // Zero, dashed; what the account keeps, plain.
        ctx.strokeStyle = chart.theme.line
        ctx.lineWidth = 1
        ctx.setLineDash([4, 3])
        ctx.beginPath()
        ctx.moveTo(0, y(0))
        ctx.lineTo(chart.width, y(0))
        ctx.stroke()
        ctx.setLineDash([])
        if (c.floor_cents !== 0) {
            ctx.strokeStyle = chart.theme.muted
            ctx.beginPath()
            ctx.moveTo(0, y(c.floor_cents))
            ctx.lineTo(chart.width, y(c.floor_cents))
            ctx.stroke()
        }
        // Without the doubtful income, lighter; then the balance as expected.
        line(c.without, 0.4, 1.5)
        line(c.points, 1, 2)
        // The incomes above their step, the large payments below theirs.
        ctx.font = "11px sans-serif"
        ctx.fillStyle = chart.theme.muted
        let lastRight = -1000
        for (const mark of c.marks) {
            const mx = x(mark.day)
            const my = y(mark.cents)
            ctx.beginPath()
            ctx.arc(mx, my, 2.5, 0, 2 * Math.PI)
            ctx.fill()
            const label = mark.label.length > 18 ? mark.label.slice(0, 17) + "…" : mark.label
            const w = ctx.measureText(label).width
            const lx = Math.max(0, Math.min(chart.width - w, mx - w / 2))
            // A name that would run over the one before is left to the list below.
            if (lx < lastRight + 6)
                continue
            ctx.globalAlpha = mark.uncertain ? 0.6 : 1
            ctx.fillText(label, lx, mark.income ? Math.max(11, my - 6) : Math.min(chart.height - 2, my + 14))
            ctx.globalAlpha = 1
            lastRight = lx + w
        }
    }
}
