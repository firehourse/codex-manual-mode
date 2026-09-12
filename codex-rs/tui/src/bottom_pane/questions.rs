//! Own question-editor creation, restoration, and the collapsed entry point.

use super::*;
use crate::wrapping::RtOptions;
use crate::wrapping::word_wrap_lines;

pub(super) struct QuestionSummary(pub(super) Vec<Line<'static>>);

impl QuestionSummary {
    fn wrapped_lines(&self, width: u16) -> Vec<Line<'static>> {
        self.0
            .iter()
            .flat_map(|line| {
                word_wrap_lines(
                    std::iter::once(line),
                    RtOptions::new(usize::from(width)).subsequent_indent("    ".into()),
                )
            })
            .collect()
    }
}

impl Renderable for QuestionSummary {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        Renderable::render(&Paragraph::new(self.wrapped_lines(area.width)), area, buf);
    }

    fn desired_height(&self, width: u16) -> u16 {
        self.wrapped_lines(width).len() as u16
    }
}

impl BottomPane {
    pub(crate) fn push_async_questions(
        &mut self,
        message_id: &str,
        questions: &[codex_protocol::items::AsyncUserInputQuestion],
    ) {
        self.question_editor().append(message_id, questions);
        self.schedule_active_view_frame();
        self.request_redraw();
    }

    pub(crate) fn clear_pending_questions(&mut self) {
        if let Some(questions) = &mut self.questions {
            questions.clear_pending();
            self.request_redraw();
        }
    }

    fn question_editor(&mut self) -> &mut AsyncQuestions {
        self.questions.get_or_insert_with(|| {
            let mut questions = AsyncQuestions::new(
                self.app_event_tx.clone(),
                self.has_input_focus,
                self.enhanced_keys_supported,
                self.disable_paste_burst,
                self.keymap.clone(),
            );
            questions.set_vim_enabled(self.composer.is_vim_enabled());
            Box::new(questions)
        })
    }

    pub(crate) fn restore_questions(&mut self, state: Option<QuestionState>) {
        if let Some(state) = state {
            self.question_editor().restore(state);
        }
        self.request_redraw();
    }

    pub(super) fn question_summary(&self, now: Instant) -> Option<Vec<Line<'static>>> {
        let questions = self
            .questions
            .as_ref()
            .filter(|q| !q.expanded && q.unanswered_count() > 0)?;
        let count = questions.unanswered_count();
        let countdown = questions
            .countdown(now)
            .map(|text| format!(" · {text}"))
            .unwrap_or_default();
        let mut lines = vec![Line::from(vec![
            "  ? ".dim(),
            Span::styled(
                format!("{count} question{}", if count == 1 { "" } else { "s" }),
                crate::style::accent_style(),
            )
            .bold(),
            countdown.dim(),
        ])];
        let hints = self
            .keymap
            .shortcut_hints(KeymapContext::Chat, "edit_queued_message");
        if !hints.is_empty() {
            let mut spans = vec!["    ".into()];
            for (index, hint) in hints.into_iter().enumerate() {
                if index > 0 {
                    spans.push(" / ".into());
                }
                spans.push(hint.into());
            }
            spans.push(" to answer".into());
            lines.push(Line::from(spans).dim());
        }
        Some(lines)
    }
}
