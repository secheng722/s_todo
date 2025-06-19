use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};
use serde::{Deserialize, Serialize};
use std::{error::Error, io, time::{SystemTime, UNIX_EPOCH}};

#[derive(Clone, Serialize, Deserialize)]
struct Todo {
    title: String,
    description: String,
    completed: bool,
    // 时间记录字段
    start_time: Option<u64>,    // 开始时间（时间戳）
    end_time: Option<u64>,      // 结束时间（时间戳）
    total_duration: u64,        // 总耗时（秒）
}

impl Todo {
    fn new(title: String) -> Self {
        Self {
            title,
            description: String::new(),
            completed: false,
            start_time: None,
            end_time: None,
            total_duration: 0,
        }
    }

    // 开始工作 - 记录开始时间
    fn start_work(&mut self) {
        self.start_time = Some(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
        );
        self.end_time = None;  // 清除结束时间
    }

    // 结束工作 - 记录结束时间并计算耗时
    fn end_work(&mut self) {
        if let Some(start) = self.start_time {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            
            self.end_time = Some(now);
            let session_duration = now - start;
            self.total_duration += session_duration;
        }
    }

    // 切换工作状态
    fn toggle_work(&mut self) {
        if self.start_time.is_some() && self.end_time.is_none() {
            // 正在工作，结束工作
            self.end_work();
        } else {
            // 没有工作或已结束，开始新的工作
            self.start_work();
        }
    }

    // 检查是否正在工作
    fn is_working(&self) -> bool {
        self.start_time.is_some() && self.end_time.is_none()
    }

    // 格式化时间显示
    fn format_duration(&self) -> String {
        let total_seconds = self.total_duration;
        
        if total_seconds == 0 {
            return String::new();
        }
        
        let months = total_seconds / 2592000;  // 30天 * 24小时 * 60分钟 * 60秒 = 2592000秒 ≈ 1个月
        let days = (total_seconds % 2592000) / 86400;  // 86400 秒 = 1 天
        let hours = (total_seconds % 86400) / 3600;
        let minutes = (total_seconds % 3600) / 60;
        let seconds = total_seconds % 60;
        
        match (months, days, hours, minutes, seconds) {
            // 有月份的情况
            (mo, d, h, _, _) if mo > 0 => {
                match (d, h) {
                    (d, h) if d > 0 && h > 0 => format!("{}mo {}d {}h", mo, d, h),
                    (d, _) if d > 0 => format!("{}mo {}d", mo, d),
                    (_, h) if h > 0 => format!("{}mo {}h", mo, h),
                    _ => format!("{}mo", mo),
                }
            },
            // 有天数的情况
            (0, d, h, m, _) if d > 0 => {
                match (h, m) {
                    (h, m) if h > 0 && m > 0 => format!("{}d {}h {}m", d, h, m),
                    (h, _) if h > 0 => format!("{}d {}h", d, h),
                    (_, m) if m > 0 => format!("{}d {}m", d, m),
                    _ => format!("{}d", d),
                }
            },
            // 有小时的情况
            (0, 0, h, m, s) if h > 0 => {
                match (m, s) {
                    (m, s) if m > 0 && s > 0 => format!("{}h {}m {}s", h, m, s),
                    (m, _) if m > 0 => format!("{}h {}m", h, m),
                    (_, s) if s > 0 => format!("{}h {}s", h, s),
                    _ => format!("{}h", h),
                }
            },
            // 有分钟的情况
            (0, 0, 0, m, s) if m > 0 => {
                if s > 0 {
                    format!("{}m {}s", m, s)
                } else {
                    format!("{}m", m)
                }
            },
            // 只有秒的情况
            (0, 0, 0, 0, s) if s > 0 => format!("{}s", s),
            // 默认情况（应该不会到达这里）
            _ => String::new(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct Project {
    name: String,
    todos: Vec<Todo>,
}

#[derive(Serialize, Deserialize)]
struct AppData {
    projects: Vec<Project>,
}

struct App {
    projects: Vec<Project>,
    project_state: ListState,
    todo_state: ListState,
    active_panel: Panel,
    input_mode: InputMode,
    input: String,
}

#[derive(PartialEq)]
enum Panel {
    Projects,
    Todos,
}

#[derive(PartialEq)]
enum InputMode {
    Normal,
    AddingProject,
    AddingTodo,
    RenamingProject,
    RenamingTodo,
}

impl App {
    fn new() -> App {
        let mut app = App {
            projects: Self::load_data(),
            project_state: ListState::default(),
            todo_state: ListState::default(),
            active_panel: Panel::Projects,
            input_mode: InputMode::Normal,
            input: String::new(),
        };

        if !app.projects.is_empty() {
            app.project_state.select(Some(0));
            app.todo_state.select(Some(0));
        }
        app
    }

    // 加载数据
    fn load_data() -> Vec<Project> {
        let data_file = Self::get_data_file_path();

        if let Ok(content) = std::fs::read_to_string(&data_file) {
            if let Ok(app_data) = serde_json::from_str::<AppData>(&content) {
                return app_data.projects;
            }
        }

        // 如果加载失败，返回默认数据
        vec![
            Project {
                name: "工作项目".to_string(),
                todos: vec![Todo::new("完成报告".to_string())],
            },
            Project {
                name: "个人学习".to_string(),
                todos: vec![Todo::new("学习 Rust".to_string())],
            },
        ]
    }

    // 保存数据
    fn save_data(&self) {
        let app_data = AppData {
            projects: self.projects.clone(),
        };

        let data_file = Self::get_data_file_path();

        // 确保目录存在
        if let Some(parent) = std::path::Path::new(&data_file).parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        if let Ok(json) = serde_json::to_string_pretty(&app_data) {
            let _ = std::fs::write(&data_file, json);
        }
    }

    // 获取数据文件路径
    fn get_data_file_path() -> String {
        if let Some(home) = std::env::var_os("HOME") {
            format!("{}/.config/s_todo/data.json", home.to_string_lossy())
        } else {
            "./s_todo_data.json".to_string()
        }
    }

    fn get_current_project(&self) -> Option<&Project> {
        self.project_state.selected().map(|i| &self.projects[i])
    }

    fn get_current_todos(&self) -> Vec<&Todo> {
        if let Some(project) = self.get_current_project() {
            project.todos.iter().collect()
        } else {
            vec![]
        }
    }

    // 获取当前选中的 todo（可变引用）
    fn get_current_todo_mut(&mut self) -> Option<&mut Todo> {
        if let (Some(project_idx), Some(todo_idx)) = 
            (self.project_state.selected(), self.todo_state.selected()) {
            self.projects.get_mut(project_idx)
                .and_then(|project| project.todos.get_mut(todo_idx))
        } else {
            None
        }
    }

    // 切换当前 todo 的计时状态
    fn toggle_current_todo_timer(&mut self) -> bool {
        if let Some(todo) = self.get_current_todo_mut() {
            todo.toggle_work();
            true
        } else {
            false
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    // 设置终端
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let app = App::new();
    let res = run_app(&mut terminal, app);

    // 恢复终端
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err);
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        if let Event::Key(key) = event::read()? {
            let mut should_save = false;

            match app.input_mode {
                InputMode::Normal => match key.code {
                    KeyCode::Char('q') => {
                        app.save_data(); // 退出前保存
                        return Ok(());
                    }
                    KeyCode::Char('s') => {
                        app.save_data();
                        continue;
                    }
                    KeyCode::Tab => {
                        app.active_panel = match app.active_panel {
                            Panel::Projects => {
                                // 切换到 Todo 面板时，确保有选中项
                                let todos = app.get_current_todos();
                                if !todos.is_empty() && app.todo_state.selected().is_none() {
                                    app.todo_state.select(Some(0));
                                }
                                Panel::Todos
                            },
                            Panel::Todos => {
                                // 切换到项目面板时，确保有选中项
                                if !app.projects.is_empty() && app.project_state.selected().is_none() {
                                    app.project_state.select(Some(0));
                                }
                                Panel::Projects
                            },
                        };
                    }
                    KeyCode::Char('j') | KeyCode::Down => match app.active_panel {
                        Panel::Projects => {
                            let i = match app.project_state.selected() {
                                Some(i) => {
                                    if i >= app.projects.len() - 1 {
                                        0
                                    } else {
                                        i + 1
                                    }
                                }
                                None => 0,
                            };
                            app.project_state.select(Some(i));
                            app.todo_state.select(Some(0));
                        }
                        Panel::Todos => {
                            let todos = app.get_current_todos();
                            if !todos.is_empty() {
                                let i = match app.todo_state.selected() {
                                    Some(i) => {
                                        if i >= todos.len() - 1 {
                                            0
                                        } else {
                                            i + 1
                                        }
                                    }
                                    None => 0,
                                };
                                app.todo_state.select(Some(i));
                            }
                        }
                    },
                    KeyCode::Char('k') | KeyCode::Up => match app.active_panel {
                        Panel::Projects => {
                            let i = match app.project_state.selected() {
                                Some(i) => {
                                    if i == 0 {
                                        app.projects.len() - 1
                                    } else {
                                        i - 1
                                    }
                                }
                                None => 0,
                            };
                            app.project_state.select(Some(i));
                            app.todo_state.select(Some(0));
                        }
                        Panel::Todos => {
                            let todos = app.get_current_todos();
                            if !todos.is_empty() {
                                let i = match app.todo_state.selected() {
                                    Some(i) => {
                                        if i == 0 {
                                            todos.len() - 1
                                        } else {
                                            i - 1
                                        }
                                    }
                                    None => 0,
                                };
                                app.todo_state.select(Some(i));
                            }
                        }
                    },
                    KeyCode::Char(' ') => {
                        if app.active_panel == Panel::Todos {
                            if let (Some(project_idx), Some(todo_idx)) =
                                (app.project_state.selected(), app.todo_state.selected())
                            {
                                let todo = &mut app.projects[project_idx].todos[todo_idx];
                                
                                // 如果正在计时且要标记为完成，自动结束计时
                                if todo.is_working() && !todo.completed {
                                    todo.end_work();
                                }
                                
                                // 切换完成状态
                                todo.completed = !todo.completed;
                                should_save = true;
                            }
                        }
                    }
                    KeyCode::Char('a') => {
                        app.input_mode = match app.active_panel {
                            Panel::Projects => InputMode::AddingProject,
                            Panel::Todos => InputMode::AddingTodo,
                        };
                        app.input.clear();
                    }
                    KeyCode::Char('t') => {
                        // 切换当前 todo 的计时状态
                        if app.active_panel == Panel::Todos && app.toggle_current_todo_timer() {
                            should_save = true;
                        }
                    }
                    KeyCode::Char('r') => {
                        // 重命名当前选中的项目或 todo
                        match app.active_panel {
                            Panel::Projects => {
                                if let Some(idx) = app.project_state.selected() {
                                    app.input_mode = InputMode::RenamingProject;
                                    app.input = app.projects[idx].name.clone();
                                }
                            }
                            Panel::Todos => {
                                if let (Some(project_idx), Some(todo_idx)) = 
                                    (app.project_state.selected(), app.todo_state.selected()) {
                                    app.input_mode = InputMode::RenamingTodo;
                                    app.input = app.projects[project_idx].todos[todo_idx].title.clone();
                                }
                            }
                        }
                    }
                    KeyCode::Char('d') => match app.active_panel {
                        Panel::Projects => {
                            if let Some(idx) = app.project_state.selected() {
                                if idx < app.projects.len() {
                                    app.projects.remove(idx);
                                    if app.projects.is_empty() {
                                        app.project_state.select(None);
                                    } else if idx >= app.projects.len() {
                                        app.project_state.select(Some(app.projects.len() - 1));
                                    }
                                    should_save = true;
                                }
                            }
                        }
                        Panel::Todos => {
                            if let (Some(project_idx), Some(todo_idx)) =
                                (app.project_state.selected(), app.todo_state.selected())
                            {
                                if todo_idx < app.projects[project_idx].todos.len() {
                                    app.projects[project_idx].todos.remove(todo_idx);
                                    let todos_len = app.projects[project_idx].todos.len();
                                    if todos_len == 0 {
                                        app.todo_state.select(None);
                                    } else if todo_idx >= todos_len {
                                        app.todo_state.select(Some(todos_len - 1));
                                    }
                                    should_save = true;
                                }
                            }
                        }
                    },
                    _ => {}
                },
                InputMode::AddingProject => match key.code {
                    KeyCode::Enter => {
                        if !app.input.is_empty() {
                            app.projects.push(Project {
                                name: app.input.clone(),
                                todos: vec![],
                            });
                            // 自动选中新添加的项目
                            let new_index = app.projects.len() - 1;
                            app.project_state.select(Some(new_index));
                            // 清空 todo 选择，因为新项目没有 todo
                            app.todo_state.select(None);
                            app.input.clear();
                            should_save = true;
                        }
                        app.input_mode = InputMode::Normal;
                    }
                    KeyCode::Char(c) => app.input.push(c),
                    KeyCode::Backspace => {
                        app.input.pop();
                    }
                    KeyCode::Esc => app.input_mode = InputMode::Normal,
                    _ => {}
                },
                InputMode::AddingTodo => match key.code {
                    KeyCode::Enter => {
                        if !app.input.is_empty() {
                            if let Some(project_idx) = app.project_state.selected() {
                                app.projects[project_idx].todos.push(Todo::new(app.input.clone()));
                                // 自动选中新添加的 todo
                                let new_todo_index = app.projects[project_idx].todos.len() - 1;
                                app.todo_state.select(Some(new_todo_index));
                                should_save = true;
                            }
                            app.input.clear();
                        }
                        app.input_mode = InputMode::Normal;
                    }
                    KeyCode::Char(c) => app.input.push(c),
                    KeyCode::Backspace => {
                        app.input.pop();
                    }
                    KeyCode::Esc => app.input_mode = InputMode::Normal,
                    _ => {}
                },
                InputMode::RenamingProject => match key.code {
                    KeyCode::Enter => {
                        if !app.input.is_empty() {
                            if let Some(idx) = app.project_state.selected() {
                                app.projects[idx].name = app.input.clone();
                                should_save = true;
                            }
                            app.input.clear();
                        }
                        app.input_mode = InputMode::Normal;
                    }
                    KeyCode::Char(c) => app.input.push(c),
                    KeyCode::Backspace => {
                        app.input.pop();
                    }
                    KeyCode::Esc => app.input_mode = InputMode::Normal,
                    _ => {}
                },
                InputMode::RenamingTodo => match key.code {
                    KeyCode::Enter => {
                        if !app.input.is_empty() {
                            if let (Some(project_idx), Some(todo_idx)) = 
                                (app.project_state.selected(), app.todo_state.selected()) {
                                app.projects[project_idx].todos[todo_idx].title = app.input.clone();
                                should_save = true;
                            }
                            app.input.clear();
                        }
                        app.input_mode = InputMode::Normal;
                    }
                    KeyCode::Char(c) => app.input.push(c),
                    KeyCode::Backspace => {
                        app.input.pop();
                    }
                    KeyCode::Esc => app.input_mode = InputMode::Normal,
                    _ => {}
                },
            }

            // 如果有修改，自动保存
            if should_save {
                app.save_data();
            }
        }
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    let terminal_width = f.area().width;

    // 根据终端宽度动态调整布局
    let (left_constraint, right_constraint) = if terminal_width < 80 {
        // 窄屏幕：垂直布局
        (Constraint::Percentage(100), Constraint::Percentage(0))
    } else if terminal_width < 120 {
        // 中等屏幕：左侧较窄
        (Constraint::Min(25), Constraint::Min(40))
    } else {
        // 宽屏幕：正常比例
        (Constraint::Percentage(30), Constraint::Percentage(70))
    };

    // 当屏幕太窄时使用垂直布局
    let chunks = if terminal_width < 80 {
        let vertical_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)].as_ref())
            .split(f.area());
        vertical_chunks
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([left_constraint, right_constraint].as_ref())
            .split(f.area())
    };

    // 左侧：项目列表
    let project_items: Vec<ListItem> = app
        .projects
        .iter()
        .map(|project| {
            let name = if chunks[0].width < 20 {
                // 极窄时只显示项目名
                if project.name.len() > chunks[0].width as usize - 5 {
                    format!(
                        "📁{}",
                        &project.name
                            [..std::cmp::min(project.name.len(), chunks[0].width as usize - 8)]
                    )
                } else {
                    format!("📁{}", project.name)
                }
            } else {
                // 正常显示
                format!("📁 {} ({})", project.name, project.todos.len())
            };
            ListItem::new(name)
        })
        .collect();

    let projects_title = if terminal_width < 80 {
        format!(
            "项目 [{}]",
            if app.active_panel == Panel::Projects {
                "选中"
            } else {
                "未选中"
            }
        )
    } else {
        "项目".to_string()
    };

    let projects_list = List::new(project_items)
        .block(
            Block::default()
                .title(projects_title)
                .borders(Borders::ALL)
                .border_style(if app.active_panel == Panel::Projects {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                }),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol(">> ");

    f.render_stateful_widget(projects_list, chunks[0], &mut app.project_state);

    // 右侧：Todo列表（如果有空间显示）
    if chunks.len() > 1 && chunks[1].width > 10 {
        let todos = app.get_current_todos();
        let todo_items: Vec<ListItem> = todos
            .iter()
            .map(|todo| {
                let status = if todo.completed { "✅" } else { "⭕" };
                let timer_indicator = if todo.is_working() { "⏱️ " } else { "" };
                let time_str = if todo.total_duration > 0 {
                    format!(" [{}]", todo.format_duration())
                } else {
                    String::new()
                };
                
                let title = if chunks[1].width < 30 {
                    // 窄屏时截断文本
                    let max_len = chunks[1].width as usize - 12;
                    if todo.title.len() > max_len {
                        format!("{} {}{}...", status, timer_indicator, &todo.title[..max_len])
                    } else {
                        format!("{} {}{}{}", status, timer_indicator, todo.title, time_str)
                    }
                } else {
                    format!("{} {}{}{}", status, timer_indicator, todo.title, time_str)
                };
                ListItem::new(title)
            })
            .collect();

        let todos_title = if terminal_width < 80 {
            format!(
                "Todo [{}]",
                if app.active_panel == Panel::Todos {
                    "选中"
                } else {
                    "未选中"
                }
            )
        } else {
            format!(
                "Todo - {}",
                app.get_current_project().map_or("无项目", |p| &p.name)
            )
        };

        let todos_list = List::new(todo_items)
            .block(
                Block::default()
                    .title(todos_title)
                    .borders(Borders::ALL)
                    .border_style(if app.active_panel == Panel::Todos {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default()
                    }),
            )
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
            .highlight_symbol(">> ");

        f.render_stateful_widget(todos_list, chunks[1], &mut app.todo_state);
    }

    // 输入框 - 调整弹窗大小
    if app.input_mode != InputMode::Normal {
        let input_title = match app.input_mode {
            InputMode::AddingProject => "添加新项目",
            InputMode::AddingTodo => "添加新Todo",
            InputMode::RenamingProject => "重命名项目",
            InputMode::RenamingTodo => "重命名Todo",
            _ => "",
        };

        let input = Paragraph::new(app.input.as_str())
            .block(Block::default().title(input_title).borders(Borders::ALL));

        // 根据终端大小调整弹窗
        let (popup_width, popup_height) = if terminal_width < 60 {
            (90, 3) // 窄屏时占更多比例
        } else {
            (60, 3) // 正常大小
        };

        let popup_area = centered_rect(popup_width, popup_height, f.area());
        f.render_widget(ratatui::widgets::Clear, popup_area);
        f.render_widget(input, popup_area);
    }

    // 在底部显示帮助信息
    if f.area().height > 5 {
        let help_text = "Tab(切换) j/k(上下) 空格(完成) a(添加) r(重命名) t(计时) d(删除) s(保存) q(退出)";
        let help_area = ratatui::layout::Rect {
            x: 0,
            y: f.area().height - 1,
            width: f.area().width,
            height: 1,
        };

        let help_paragraph = Paragraph::new(help_text).style(Style::default().fg(Color::Gray));

        f.render_widget(help_paragraph, help_area);
    }
}

fn centered_rect(percent_x: u16, height: u16, r: ratatui::layout::Rect) -> ratatui::layout::Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length((r.height - height) / 2),
            Constraint::Length(height),
            Constraint::Length((r.height - height) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
