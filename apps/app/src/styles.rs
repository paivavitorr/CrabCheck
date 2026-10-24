pub const GLOBAL_CSS: &str = r#"
    * { box-sizing: border-box; -webkit-tap-highlight-color: transparent; }
    html, body {
        margin: 0;
        padding: 0;
        background: #F6F7F9;
        color: #101828;
        font-family: -apple-system, BlinkMacSystemFont, "Inter", "Segoe UI", Roboto, sans-serif;
        -webkit-font-smoothing: antialiased;
    }
    .app-shell {
        min-height: 100vh;
        max-width: 520px;
        margin: 0 auto;
        padding: 0 0 84px 0;
        background: #F6F7F9;
    }
    .topbar {
        position: sticky;
        top: 0;
        z-index: 20;
        background: rgba(255, 255, 255, 0.94);
        backdrop-filter: blur(10px);
        border-bottom: 1px solid #E4E7EC;
        padding: 13px 16px;
        display: flex;
        justify-content: space-between;
        align-items: center;
    }
    .brand-row {
        display: flex;
        align-items: center;
        gap: 9px;
    }
    .brand-logo-img {
        width: 26px;
        height: 26px;
        border-radius: 6px;
        object-fit: contain;
    }
    .brand-name {
        margin: 0;
        font-size: 16px;
        font-weight: 600;
        letter-spacing: -0.02em;
        color: #101828;
    }
    .health-pill {
        display: inline-flex;
        align-items: center;
        gap: 6px;
        padding: 5px 12px;
        border-radius: 999px;
        font-size: 12px;
        font-weight: 500;
        background: #F2F4F7;
        color: #344054;
        border: 1px solid #E4E7EC;
        cursor: pointer;
        transition: transform 0.1s ease, background 0.15s ease, border-color 0.15s ease, box-shadow 0.15s ease;
    }
    .health-pill:hover {
        background: #EAECF0;
        border-color: #D0D5DD;
    }
    .health-pill:active {
        transform: scale(0.95);
        background: #E4E7EC;
    }
    .dot-ok {
        width: 6px;
        height: 6px;
        border-radius: 50%;
        background: #12B76A;
    }
    .dot-warn {
        width: 6px;
        height: 6px;
        border-radius: 50%;
        background: #F79009;
    }
    .page-body {
        padding: 16px;
    }
    .surface-group {
        background: #FFFFFF;
        border: 1px solid #E4E7EC;
        border-radius: 12px;
        overflow: hidden;
        margin-bottom: 16px;
    }
    .surface-pad {
        background: #FFFFFF;
        border: 1px solid #E4E7EC;
        border-radius: 12px;
        padding: 14px;
        margin-bottom: 16px;
    }
    .list-row {
        padding: 13px 14px;
        border-bottom: 1px solid #F2F4F7;
        display: flex;
        justify-content: space-between;
        align-items: center;
        gap: 12px;
    }
    .list-row:last-child {
        border-bottom: none;
    }
    .section-head {
        display: flex;
        justify-content: space-between;
        align-items: baseline;
        margin: 20px 2px 8px 2px;
    }
    .section-head:first-child {
        margin-top: 4px;
    }
    .section-title {
        font-size: 13px;
        font-weight: 600;
        color: #475467;
        margin: 0;
    }
    .section-meta {
        font-size: 12px;
        color: #667085;
    }
    .time-display {
        font-variant-numeric: tabular-nums;
        font-size: 25px;
        font-weight: 600;
        letter-spacing: -0.03em;
        color: #101828;
        line-height: 1;
        min-width: 66px;
    }
    .field-input {
        width: 100%;
        padding: 9px 11px;
        background: #FFFFFF;
        border: 1px solid #D0D5DD;
        border-radius: 8px;
        font-size: 14px;
        color: #101828;
        outline: none;
    }
    .btn-primary {
        padding: 9px 14px;
        background: #101828;
        color: #FFFFFF;
        border: 1px solid #101828;
        border-radius: 8px;
        font-size: 13px;
        font-weight: 500;
        cursor: pointer;
        transition: transform 0.1s cubic-bezier(0.4, 0, 0.2, 1), background 0.15s ease, filter 0.12s ease, box-shadow 0.15s ease;
    }
    .btn-primary:hover {
        background: #1D2939;
        border-color: #1D2939;
    }
    .btn-primary:active {
        transform: scale(0.96);
        filter: brightness(0.88);
    }
    .btn-secondary {
        padding: 6px 11px;
        background: #FFFFFF;
        color: #344054;
        border: 1px solid #D0D5DD;
        border-radius: 7px;
        font-size: 12px;
        font-weight: 500;
        cursor: pointer;
        transition: transform 0.1s cubic-bezier(0.4, 0, 0.2, 1), background 0.15s ease, border-color 0.15s ease, box-shadow 0.15s ease;
    }
    .btn-secondary:hover {
        background: #F9FAFB;
        border-color: #98A2B3;
        color: #101828;
    }
    .btn-secondary:active {
        transform: scale(0.95);
        background: #F2F4F7;
        border-color: #98A2B3;
    }
    .btn-danger-ghost {
        padding: 6px 11px;
        background: #FFFFFF;
        color: #B42318;
        border: 1px solid #FDA29B;
        border-radius: 7px;
        font-size: 12px;
        font-weight: 500;
        cursor: pointer;
        transition: transform 0.1s cubic-bezier(0.4, 0, 0.2, 1), background 0.15s ease, border-color 0.15s ease;
    }
    .btn-danger-ghost:hover {
        background: #FEF3F2;
        border-color: #F04438;
        color: #912018;
    }
    .btn-danger-ghost:active {
        transform: scale(0.95);
        background: #FEE4E2;
        border-color: #D92D20;
    }
    .bottom-nav {
        position: fixed;
        bottom: 0;
        left: 0;
        right: 0;
        height: 62px;
        background: rgba(255, 255, 255, 0.96);
        backdrop-filter: blur(10px);
        border-top: 1px solid #E4E7EC;
        display: flex;
        justify-content: space-around;
        align-items: center;
        z-index: 30;
        max-width: 520px;
        margin: 0 auto;
    }
    .nav-item {
        flex: 1;
        height: 100%;
        background: transparent;
        border: none;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        gap: 4px;
        font-size: 11px;
        font-weight: 500;
        color: #667085;
        cursor: pointer;
        transition: transform 0.1s ease, color 0.15s ease;
    }
    .nav-item:hover {
        color: #101828;
    }
    .nav-item:active {
        transform: scale(0.92);
        color: #101828;
    }
    .nav-item-active {
        color: #101828;
        font-weight: 600;
    }
    .cal-card {
        background: #FFFFFF;
        border: 1px solid #E4E7EC;
        border-radius: 12px;
        padding: 12px;
        margin-bottom: 14px;
    }
    .cal-top-row {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 10px;
    }
    .cal-title-btn {
        background: transparent;
        border: none;
        padding: 4px 6px;
        font-size: 14px;
        font-weight: 600;
        color: #101828;
        display: inline-flex;
        align-items: center;
        gap: 6px;
        cursor: pointer;
        border-radius: 6px;
    }
    .cal-nav-group {
        display: flex;
        align-items: center;
        gap: 6px;
    }
    .cal-btn-today {
        padding: 4px 9px;
        background: #F2F4F7;
        color: #344054;
        border: 1px solid #D0D5DD;
        border-radius: 6px;
        font-size: 11px;
        font-weight: 600;
        cursor: pointer;
    }
    .cal-nav-arrow {
        width: 28px;
        height: 28px;
        display: inline-flex;
        align-items: center;
        justify-content: center;
        background: #FFFFFF;
        border: 1px solid #D0D5DD;
        border-radius: 6px;
        color: #344054;
        font-size: 11px;
        cursor: pointer;
    }
    .week-strip-grid {
        display: grid;
        grid-template-columns: repeat(7, 1fr);
        gap: 4px;
    }
    .cal-day-cell {
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        padding: 6px 2px;
        border-radius: 8px;
        background: transparent;
        border: 1px solid transparent;
        cursor: pointer;
        position: relative;
    }
    .cal-day-weekday {
        font-size: 10px;
        font-weight: 500;
        color: #667085;
        text-transform: uppercase;
        letter-spacing: 0.02em;
        margin-bottom: 3px;
    }
    .cal-day-num {
        font-size: 14px;
        font-weight: 600;
        font-variant-numeric: tabular-nums;
        color: #101828;
        width: 26px;
        height: 26px;
        display: flex;
        align-items: center;
        justify-content: center;
        border-radius: 50%;
    }
    .cal-day-cell.is-selected .cal-day-num {
        background: #101828;
        color: #FFFFFF;
    }
    .cal-day-cell.is-selected .cal-day-weekday {
        color: #101828;
        font-weight: 600;
    }
    .cal-day-cell.is-today:not(.is-selected) .cal-day-num {
        border: 1.5px solid #101828;
        color: #101828;
    }
    .cal-dot {
        width: 4px;
        height: 4px;
        border-radius: 50%;
        margin-top: 2px;
    }
    .cal-dot-ok {
        background: #12B76A;
    }
    .cal-dot-fail {
        background: #F04438;
    }
    .cal-dot-snooze {
        background: #F79009;
    }
    .cal-month-table {
        display: grid;
        grid-template-columns: repeat(7, 1fr);
        gap: 4px;
        margin-top: 6px;
    }
    .cal-weekday-header {
        font-size: 10px;
        font-weight: 600;
        color: #98A2B3;
        text-align: center;
        padding-bottom: 4px;
        text-transform: uppercase;
    }
    .cal-month-cell {
        aspect-ratio: 1;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        border-radius: 8px;
        background: transparent;
        border: 1px solid transparent;
        font-size: 13px;
        font-weight: 500;
        font-variant-numeric: tabular-nums;
        color: #101828;
        cursor: pointer;
        position: relative;
    }
    .cal-month-cell.is-selected {
        background: #101828;
        color: #FFFFFF;
        font-weight: 600;
    }
    .cal-month-cell.is-today:not(.is-selected) {
        border: 1.5px solid #101828;
        font-weight: 600;
    }
    .cal-month-cell.is-empty {
        cursor: default;
    }
    .streak-banner {
        background: #FFFFFF;
        border: 1px solid #E4E7EC;
        border-radius: 12px;
        padding: 12px 14px;
        margin-bottom: 14px;
        display: flex;
        justify-content: space-between;
        align-items: center;
    }
    .streak-stat-col {
        display: flex;
        flex-direction: column;
        align-items: center;
    }
    .streak-stat-val {
        font-size: 18px;
        font-weight: 700;
        font-variant-numeric: tabular-nums;
        color: #101828;
    }
    .streak-stat-label {
        font-size: 10px;
        font-weight: 600;
        text-transform: uppercase;
        color: #667085;
        letter-spacing: 0.03em;
        margin-top: 1px;
    }
"#;
