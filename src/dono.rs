use crate::dono::query::ContributionLevel;
use crate::utils::color::*;
use crate::Config;

use anyhow::{anyhow, Error, Result};
use graphql_client::{reqwest::post_graphql_blocking as post_graphql, GraphQLQuery};
use reqwest::{blocking::Client, header};

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/graphql/schema.gql",
    query_path = "src/graphql/query.gql",
    response_derives = "Debug"
)]
struct Query;

type Date = String;

pub struct Contribution {
    pub date: String,
    pub count: i64,
    pub color: String,
    pub contribution_level: ContributionLevel,
}

const GITHUB_URI: &str = "https://api.github.com/graphql";

pub struct Dono {
    config: Config,
}

impl Dono {
    pub fn new(config: Config) -> Self {
        if let Err(err) = config.validate() {
            eprintln!("{err}");
            std::process::exit(1);
        }

        Dono { config }
    }

    pub fn get_contributions(&self, user_name: String) -> Vec<Contribution> {
        let vars = query::Variables { user_name };

        match self.post_query(vars) {
            Ok(response) => response
                .contributions_collection
                .contribution_calendar
                .weeks
                .into_iter()
                .flat_map(|week| week.contribution_days)
                .map(|day| Contribution {
                    date: day.date,
                    count: day.contribution_count,
                    color: day.color,
                    contribution_level: day.contribution_level,
                })
                .collect(),
            Err(e) => {
                eprintln!("Error: {e}");
                vec![]
            }
        }
    }

    pub fn print_contributions(&self, contributions: Vec<Contribution>) {
        let months = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        let weeks = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

        // print total contributions by user
        println!(
            "\n{} {}\n",
            Style::new().bold().paint("Total contributions:"),
            self.total_contributions(&contributions)
        );

        // print month header and loop through contributions
        println!(" {} {}", " ".repeat(5), months.join("\t"));
        for (i, week) in weeks.iter().enumerate() {
            print!("{week} ");
            for (j, contribution) in contributions.iter().enumerate() {
                if j % 7 == i {
                    if self.config.settings.native_colors {
                        self.print_native(contribution)
                    } else {
                        self.print_ansi(contribution)
                    }
                }
            }
            println!();
        }
    }

    // if 'native_colors' is set to true, print the color given by GitHub API
    fn print_native(&self, contribution: &Contribution) {
        let empty = &self.config.settings.empty;
        let fill = &self.config.settings.fill;

        // print black if count is 0, otherwise colour would be too bright
        match contribution.count {
            0 => print!("{} ", Color::Black.paint(empty)),
            _ => print!("{} ", Color::hex_to_rgb(&contribution.color).paint(fill)),
        }
    }

    // custom colors that are set in the config file
    fn print_ansi(&self, contribution: &Contribution) {
        let empty = &self.config.settings.empty;
        let fill = &self.config.settings.fill;

        // determine total contributions by contribution level
        let rgb = match &contribution.contribution_level {
            ContributionLevel::FIRST_QUARTILE => Color::hex_to_rgb(&self.config.colors.low),
            ContributionLevel::SECOND_QUARTILE => Color::hex_to_rgb(&self.config.colors.medium),
            ContributionLevel::THIRD_QUARTILE => Color::hex_to_rgb(&self.config.colors.high),
            ContributionLevel::FOURTH_QUARTILE => Color::hex_to_rgb(&self.config.colors.max),
            _ => Color::hex_to_rgb(&self.config.colors.empty),
        };

        // which symbol to print
        match contribution.count {
            0 => print!("{} ", rgb.paint(empty)),
            _ => print!("{} ", rgb.paint(fill)),
        }
    }

    fn total_contributions(&self, contributions: &[Contribution]) -> i64 {
        contributions.iter().map(|c| c.count).sum()
    }

    fn post_query(&self, vars: query::Variables) -> Result<query::QueryUser, Error> {
        let github_token = &self.config.settings.github_user_token;
        let client = Client::builder()
            .user_agent("grapql-rust/0.11.0")
            .default_headers(
                std::iter::once((
                    header::AUTHORIZATION,
                    header::HeaderValue::from_str(&format!("Bearer {github_token}")).unwrap(),
                ))
                .collect(),
            )
            .build()?;

        let response_body = post_graphql::<Query, _>(&client, GITHUB_URI, vars)?;

        let user_data = match response_body.data {
            Some(data) => data.user.ok_or(anyhow!("User not found!"))?,
            None => return Err(anyhow!("Unable to retrieve data!")),
        };

        Ok(user_data)
    }
}