// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A budget's balance over time: what happened as a line, what is planned as
// the same line, dotted; the target as a quiet dashed line, zero as a thin one.

import QtQuick

Canvas {
    id: chart

    required property var theme
    property var points: []
    property real low: 0
    property real high: 0
    property real target: 0

    onPointsChanged: chart.requestPaint()
    onWidthChanged: chart.requestPaint()
    onHeightChanged: chart.requestPaint()

    onPaint: {
        const ctx = chart.getContext("2d")
        ctx.reset()
        if (chart.points.length < 2)
            return
        const pad = 6
        const bottom = 18
        const span = Math.max(1, chart.high - chart.low)
        const x = i => pad + (chart.width - 2 * pad) * i / (chart.points.length - 1)
        const y = cents => pad + (chart.height - bottom - 2 * pad) * (1 - (cents - chart.low) / span)
        // Zero and the target.
        ctx.lineWidth = 1
        if (chart.low < 0 && chart.high > 0) {
            ctx.strokeStyle = chart.theme.line
            ctx.beginPath()
            ctx.moveTo(pad, y(0))
            ctx.lineTo(chart.width - pad, y(0))
            ctx.stroke()
        }
        ctx.strokeStyle = chart.theme.muted
        ctx.setLineDash([4, 4])
        ctx.beginPath()
        ctx.moveTo(pad, y(chart.target))
        ctx.lineTo(chart.width - pad, y(chart.target))
        ctx.stroke()
        // What happened, then what is planned, dotted.
        ctx.lineWidth = 2
        ctx.strokeStyle = chart.theme.accent
        for (const future of [false, true]) {
            ctx.setLineDash(future ? [2, 4] : [])
            ctx.beginPath()
            let started = false
            for (let i = 0; i < chart.points.length; ++i) {
                const p = chart.points[i]
                // The planned part starts from the last point that happened.
                const wanted = future ? (p.future || (i + 1 < chart.points.length && chart.points[i + 1].future)) : !p.future
                if (!wanted) {
                    started = false
                    continue
                }
                if (started)
                    ctx.lineTo(x(i), y(p.cents))
                else
                    ctx.moveTo(x(i), y(p.cents))
                started = true
            }
            ctx.stroke()
        }
        // A few labels under the line, never crowded.
        ctx.setLineDash([])
        ctx.fillStyle = chart.theme.muted
        ctx.font = "11px sans-serif"
        ctx.textAlign = "center"
        const every = Math.max(1, Math.ceil(chart.points.length / Math.max(1, Math.floor(chart.width / 48))))
        for (let i = 0; i < chart.points.length; i += every)
            ctx.fillText(chart.points[i].label, x(i), chart.height - 4)
    }
}
