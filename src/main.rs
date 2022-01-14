use std::{collections::*, env, sync::Arc};

use serenity::prelude::*;
use serenity::framework::standard::{
    macros::{command, group},
    CommandResult, StandardFramework,
};
use serenity::model::{channel::*, event::*, gateway::*, guild::*, id::*, user::User};
use serenity::utils::Content;
use serenity::{
    async_trait, client::bridge::gateway::ShardManager, client::*, http::Http, prelude::*,
};
use serenity_utils::prelude::*;

use colored::*;

pub struct ShardManagerContainer;

impl TypeMapKey for ShardManagerContainer {
    type Value = Arc<Mutex<ShardManager>>;
}

struct Handler;

// MARC IDS - Need to be changed if roles/server is changed

#[cfg(not(debug_assertions))]
const MARC_SERVER_ID: u64 = 745480681672540251;

#[cfg(debug_assertions)]
const MARC_SERVER_ID: u64 = 526941847373742112;

const NOVICE_ROCKETEER_NAME: &str = "Novice Rocketeer";
const ALUMNI_NAME: &str = "Alumni";
const FACULTY_NAME: &str = "Mudd Admin";


// Different coloured print functions
// Just for cosmetic purposes, but it does look very nice

fn print_info(string: &str) {
    println!("{}    █ {}", "INFO".green().bold(), string.normal());
}

fn print_status(string: &str) {
    println!("{}  █ {}", "STATUS".cyan().bold(), string.normal());
}

fn print_echo(msg: &Message) {
    let message: String = String::from(&msg.content);
    let mut author_name: String = String::from(&msg.author.name);

    let author_member_in_guild: bool = msg.member.as_ref().is_some();

    if author_member_in_guild {
        let author_member_has_nick: bool = msg.member.as_ref().unwrap().nick.is_some();

        if author_member_has_nick {
            author_name = String::from(msg.member.as_ref().unwrap().nick.as_ref().unwrap());
        }
    }

    println!(
        "{}    █ {} : {}",
        "ECHO".blue().bold(),
        author_name.bold(),
        message.normal()
    );
}

fn print_command(msg: &Message) {
    println!(
        "{} █ [{}] {} {}#{}",
        "COMMAND".yellow().bold(),
        &msg.content.purple(),
        "by".yellow().italic(),
        &msg.author.name,
        &msg.author.discriminator
    );
}

fn print_error(msg: &str) {
    println!("{}   █ {}", "ERROR".red().bold(), msg);
}

// Function to send a message to a channel safely
async fn send_msg(msg: &Message, ctx: &Context, content: String) {
    if let Err(why) = &msg.channel_id.say(&ctx.http, content).await {
        print_error(&format!("Could not send message: {:?}", why));
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        print_status(&format!("Connected as {}", ready.user.name));
    }

    async fn resume(&self, _: Context, _: ResumedEvent) {
        print_info("Resumed");
    }

    async fn message(&self, ctx: Context, msg: Message) {
        // Check if it's a private message
        if msg.is_private() && &msg.author.id != &ctx.cache.current_user().await.id {
            print_echo(&msg);
            
            // Check what type of role the user has
            let mut role_type: String = String::from("");
            let guild = &ctx.cache.guild(MARC_SERVER_ID).await.unwrap();
            let mut member = guild.member(&ctx.http,msg.author.id).await.unwrap();
            let roles = member.roles(&ctx.cache).await.unwrap();

            for role in roles {
                if role.name == NOVICE_ROCKETEER_NAME {
                    role_type = String::from(NOVICE_ROCKETEER_NAME);
                } else if role.name == ALUMNI_NAME {
                    role_type = String::from(ALUMNI_NAME);
                } else if role.name == FACULTY_NAME {
                    role_type = String::from(FACULTY_NAME);
                }
            }

            print_status(&format!("DM from {}, who has {} role", &msg.author.name, role_type));

            // Now that we have what type of role, we can process the message
            if !msg.content.trim().is_empty() {
                let msg_split: Vec<&str> = msg.content.split(" ").collect();
                let mut success = true;

                if msg_split.len() == 1 {
                    if role_type != FACULTY_NAME {
                        let result = compute_adding_year(&ctx, &msg, &msg_split[0]).await;
                        if result.is_none() { success = false; }

                    } else {
                        let result = compute_adding_level(&ctx, &msg, &msg_split[0]).await;
                        if result.is_none() { success = false; }

                    }
                } else if msg_split.len() == 2 {
                    if role_type == ALUMNI_NAME {
                        let result1 = compute_adding_year(&ctx, &msg, &msg_split[0]).await;
                        let result2 = compute_adding_level(&ctx, &msg, &msg_split[1]).await;

                        if result1.is_none() || result2.is_none() { success = false; }

                    } else {
                        let _ = &msg.reply(&ctx, "Too many arguments!").await;
                        success = false;
                    }
                }

                if success {
                    let _ = &msg.reply(&ctx, "Roles successfully added! Thank you!").await;
                }
            }
        }
    }

    async fn reaction_add(&self, ctx: Context, add_reaction: Reaction) {
        let msg_user = &add_reaction.message(&ctx.http).await.unwrap().author;
        let emoji = &add_reaction.emoji;
        let bot_id = &ctx.cache.current_user().await.id;

        // Check if it's a reaction on the message from the bot
        if &msg_user.id == bot_id && &add_reaction.user_id.unwrap() != bot_id {
            let user = add_reaction.user_id.unwrap().to_user(&ctx.http).await.unwrap();

            // Remove the emoji
            if let Err(why) = add_reaction.delete(&ctx.http).await {
                print_error(&format!("Could not remove reaction: {:?}", why));
            }

            print_info(format!("Emoji reaction: {}", emoji).as_str());

            let student_emoji = ReactionType::try_from("🏫").unwrap();
            let alumni_emoji = ReactionType::try_from("🎓").unwrap();
            let faculty_emoji = ReactionType::try_from("🏢").unwrap();

            // Send DM if emoji is correct
            if emoji == &student_emoji {
                print_status("Sending student DM");
                start_student_chain(ctx, &user).await;
            } else if emoji == &alumni_emoji {
                print_status("Sending alumni DM");
                start_alumni_chain(ctx, &user).await;
            } else if emoji == &faculty_emoji {
                print_status("Sending faculty DM");
                start_faculty_chain(ctx, &user).await;
            }
        }
    }
}

async fn compute_adding_year(ctx: &Context, msg: &Message, year_str: &str) -> Option<()> {
    let year = year_str.parse::<u64>();

    if matches!(year, Ok(year) if year > 1900 && year < 9999) {
        let chars: Vec<char> = year_str.chars().collect();

        let role_name = format!("Class of '{}{}", chars[2], chars[3]);

        add_create_role(&ctx, &msg.author, role_name.as_str()).await;

        return Some(());

    } else {
        let _ = msg.reply(&ctx.http, "Please specify a valid year!").await;

        return None;
    }
}

async fn compute_adding_level(ctx: &Context, msg: &Message, level_str: &str) -> Option<()> {
    let level_str = level_str.to_lowercase();
    if level_str.contains("l") {
        let level = level_str.replace("l", "").parse::<u64>();

        if matches!(level, Ok(l) if l <= 3 && l >= 1) {
            let level = level.unwrap();

            let role_name = format!("Level {} Rocketeer", level);

            add_create_role(&ctx, &msg.author, role_name.as_str()).await;

            return Some(());

        } else {
            let _ = msg.reply(&ctx.http, "Please specify a valid level!").await;

            return None;
        }
    } else {
        if !level_str.to_lowercase().contains("none") {
            let _ = msg.reply(&ctx.http, "Please provide your level cert").await;

            return None;
        } else {
            return Some(());
        }
    }
}

async fn add_role_by_name(ctx: &Context, user: &User, role_name: &str) {
    print_status(&format!("Adding role {} to {}", role_name, user.name));
    let guild = Guild::get(&ctx.http, MARC_SERVER_ID).await.unwrap();

    let mut member = guild.member(&ctx.http,user.id).await.unwrap();
    let role = guild.role_by_name(role_name).unwrap();

    if !member.roles.contains(&role.id) {
        if let Err(why) = member.add_role(&ctx.http, role).await {
            print_error(&format!("Could not add role: {:?}", why));
        }
    }
}

async fn remove_role_by_name(ctx: &Context, user: &User, role_name: &str) {
    print_status(&format!("Removing role {} from {}", role_name, user.name));
    let guild = Guild::get(&ctx.http, MARC_SERVER_ID).await.unwrap();

    let mut member = guild.member(&ctx.http,user.id).await.unwrap();
    let role = guild.role_by_name(role_name).unwrap();

    if member.roles.contains(&role.id) {
        if let Err(why) = member.remove_role(&ctx.http, role).await {
            print_error(&format!("Could not remove role: {:?}", why));
        }
    }
}

async fn add_create_role(ctx: &Context, user: &User, role_name: &str) {
    let guild = Guild::get(&ctx.http, MARC_SERVER_ID).await.unwrap();

    let mut member = guild.member(&ctx.http,user.id).await.unwrap();

    if guild.role_by_name(&role_name).is_none() {
        if let Err(why) = guild.create_role(&ctx.http, |r| r.hoist(true).name(&role_name)).await {
            print_error(&format!("Could not create role: {:?}", why));
        }
    } 

    let guild = Guild::get(&ctx.http, MARC_SERVER_ID).await.unwrap();

    let role = guild.role_by_name(role_name).unwrap();

    if !member.roles.contains(&role.id) {
        if let Err(why) = member.add_role(&ctx.http, role).await {
            print_error(&format!("Could not add role: {:?}", why));
        }
    }
}

async fn start_student_chain(ctx: Context, user: &User) {
    // Assign role as novice rocketeer
    add_role_by_name(&ctx, user, NOVICE_ROCKETEER_NAME, ).await;
    // Remove alumni & faculty roles
    remove_role_by_name(&ctx, user, ALUMNI_NAME).await;
    remove_role_by_name(&ctx, user, FACULTY_NAME).await;

    let dm = user.direct_message(&ctx, |m| {
        m.content("Welcome again! Please respond with your expected year of graduation, formatted as \"YYYY\".")
    })
    .await;
}

async fn start_alumni_chain(ctx: Context, user: &User) {
    add_role_by_name( &ctx, user, ALUMNI_NAME).await;

    // Remove student & faculty roles
    remove_role_by_name(&ctx, user, NOVICE_ROCKETEER_NAME).await;
    remove_role_by_name(&ctx, user, FACULTY_NAME).await;

    let dm = user.direct_message(&ctx, |m| {
        m.content("Welcome again! Please respond with your year of graduation & level of rocketry certification, formatted as \"YYYY L*\". Example: \"2019 L2\". If you do not have a certification, please just respond with your year.")
    })
    .await;
}

async fn start_faculty_chain(ctx: Context, user: &User) {
    add_role_by_name(&ctx, user, FACULTY_NAME).await;

    // Remove student & alumni roles
    remove_role_by_name(&ctx, user, NOVICE_ROCKETEER_NAME).await;
    remove_role_by_name(&ctx, user, ALUMNI_NAME).await;

    let dm = user.direct_message(&ctx, |m| {
        m.content("Welcome again! Please respond with your level of rocketry certification, which can be: None, L1, L2, or L3")
    })
    .await;
}

#[command]
async fn setup(ctx: &Context, msg: &Message) -> CommandResult {
    // Log the command
    print_command(&msg);
    // First, send welcome message as embed to channel
    let r = &msg.channel_id.send_message(&ctx.http, |m| {    
        m.embed(|mut e| {
            e.title("- Welcome to the MARC Discord Server! -");
            e.description("\n**Please react to this message with**\n\n🏫  for current students\n\n🎓  for alumni\n\n🏢  for faculty\n\n\n*If you have disabled DMs from server members, you will need to enable them in your privacy settings*");
    
            e
        });
    
        m
    }).await;

    if r.is_err() {
        print_error("Could not send welcome message");
    }

    let r = r.clone().as_ref().unwrap();

    // Delete setup message
    msg.delete(&ctx).await.unwrap();

    // Add reactions
    let _ = r.react(&ctx, '🏫').await;
    let _ = r.react(&ctx, '🎓').await;
    let _ = r.react(&ctx, '🏢').await;

    Ok(())
}

#[group]
#[commands(setup)]
struct General;

#[tokio::main]
async fn main() {
    print_info("Starting up...");

    // Setup the async hashmap to store BloodGuild structs
    let framework = StandardFramework::new().configure(|c| c.prefix("!"))
        .group(&GENERAL_GROUP);


    // Login with a bot token from the environment
    let token = env::var("MARC_TOKEN").expect("Please set your MARC_TOKEN!");

    let mut client = Client::builder(token)
        .event_handler(Handler)
        .framework(framework)
        .await
        .expect("Error creating client");

    print_info("Started!");
    // start listening for events by starting a single shard
    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {:?}", why);
    }
}
